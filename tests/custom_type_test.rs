// Test that custom types work without any trait implementations
use dagx::{task, DagRunner, TaskHandle};
use futures::FutureExt;

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
    let dag = DagRunner::new();

    let fetch = dag.add_task(FetchPerson);
    let process = dag.add_task(ProcessPerson).depends_on(fetch);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let result = dag.get(process).unwrap();
    assert_eq!(result, "Alice is 30 years old");
}

#[tokio::test]
async fn test_custom_type_passthrough() {
    let dag = DagRunner::new();

    let fetch: TaskHandle<_> = dag.add_task(FetchPerson).into();
    let transform = dag.add_task(PersonToCompany).depends_on(fetch);
    let process = dag.add_task(ProcessPerson).depends_on(fetch);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let person_result = dag.get(process).unwrap();
    let company_result = dag.get(transform).unwrap();

    assert_eq!(person_result.as_str(), "Alice is 30 years old");
    assert_eq!(company_result.name, "Alice's Company");
    assert_eq!(company_result.employees.len(), 1);
}

#[tokio::test]
async fn test_custom_type_diamond_pattern() {
    let dag = DagRunner::new();

    // Source
    let fetch: TaskHandle<_> = dag.add_task(FetchPerson).into();

    // Two paths from source
    let path_a = dag.add_task(ProcessPerson).depends_on(fetch);
    let path_b = dag.add_task(PersonToCompany).depends_on(fetch);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let result_a = dag.get(path_a).unwrap();
    let result_b = dag.get(path_b).unwrap();

    assert_eq!(result_a.as_str(), "Alice is 30 years old");
    assert_eq!(result_b.name, "Alice's Company");
}

#[tokio::test]
async fn test_custom_type_fan_out() {
    let dag = DagRunner::new();

    let fetch: TaskHandle<_> = dag.add_task(FetchPerson).into();
    let process1 = dag.add_task(ProcessPerson).depends_on(fetch);
    let process2 = dag.add_task(ProcessPerson).depends_on(fetch);
    let process3 = dag.add_task(ProcessPerson).depends_on(fetch);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap))
        .await
        .unwrap();

    let result1 = dag.get(process1).unwrap();
    let result2 = dag.get(process2).unwrap();
    let result3 = dag.get(process3).unwrap();

    assert_eq!(result1, "Alice is 30 years old");
    assert_eq!(result2, "Alice is 30 years old");
    assert_eq!(result3, "Alice is 30 years old");
}
