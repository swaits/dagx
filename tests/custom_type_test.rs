// Test that custom types work with the #[task] macro
use dagx::{task, DagRunner};

#[derive(Clone, Debug, PartialEq)]
struct Person {
    name: String,
    age: u32,
}

#[derive(Clone, Debug, PartialEq)]
struct Company {
    name: String,
    employees: Vec<Person>,
}

// Source task returning custom type
struct FetchPerson;

#[task]
impl FetchPerson {
    async fn run(&self) -> Person {
        Person {
            name: "Alice".to_string(),
            age: 30,
        }
    }
}

// Task consuming custom type
struct ProcessPerson;

#[task]
impl ProcessPerson {
    async fn run(person: &Person) -> String {
        format!("{} is {} years old", person.name, person.age)
    }
}

// Task transforming custom type to another custom type
struct PersonToCompany;

#[task]
impl PersonToCompany {
    async fn run(person: &Person) -> Company {
        Company {
            name: format!("{}'s Company", person.name),
            employees: vec![person.clone()],
        }
    }
}

#[tokio::test]
async fn test_custom_type_single_dependency() {
    let mut dag = DagRunner::new();

    let fetch = dag.add_task(FetchPerson);
    let process = dag.add_task(ProcessPerson).depends_on(fetch);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    let result = output.get(process).unwrap();
    assert_eq!(result, "Alice is 30 years old");
}

#[tokio::test]
async fn test_custom_type_passthrough() {
    let mut dag = DagRunner::new();

    let fetch = dag.add_task(FetchPerson);
    let transform = dag.add_task(PersonToCompany).depends_on(fetch);
    let process = dag.add_task(ProcessPerson).depends_on(fetch);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    let person_result = output.get(process).unwrap();
    let company_result = output.get(transform).unwrap();

    assert_eq!(person_result.as_str(), "Alice is 30 years old");
    assert_eq!(company_result.name, "Alice's Company");
    assert_eq!(company_result.employees.len(), 1);
}

#[tokio::test]
async fn test_custom_type_fan_out() {
    let mut dag = DagRunner::new();

    let fetch = dag.add_task(FetchPerson);
    let process1 = dag.add_task(ProcessPerson).depends_on(fetch);
    let process2 = dag.add_task(ProcessPerson).depends_on(fetch);
    let process3 = dag.add_task(ProcessPerson).depends_on(fetch);

    let mut output = dag
        .run(|fut| async move { tokio::spawn(fut).await.unwrap() })
        .await
        .unwrap();

    let result1 = output.get(process1).unwrap();
    let result2 = output.get(process2).unwrap();
    let result3 = output.get(process3).unwrap();

    assert_eq!(result1, "Alice is 30 years old");
    assert_eq!(result2, "Alice is 30 years old");
    assert_eq!(result3, "Alice is 30 years old");
}
