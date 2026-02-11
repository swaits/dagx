//! Tree pattern DAG tests

use crate::common::task_fn;
use dagx::{DagResult, DagRunner, TaskHandle};
use futures::FutureExt;

#[tokio::test]
async fn test_binary_tree_reduction() -> DagResult<()> {
    let dag = DagRunner::new();

    // Create 8 leaf nodes
    let leaves: Vec<_> = (0..8)
        .map(|i| dag.add_task(task_fn::<(), _, _>(move |_: ()| 1 << i)))
        .collect();

    // Build binary tree reduction
    let mut current_level: Vec<dagx::TaskHandle<i32>> =
        leaves.into_iter().map(|t| t.into()).collect();

    while current_level.len() > 1 {
        let mut next_level = Vec::new();

        for chunk in current_level.chunks(2) {
            if chunk.len() == 2 {
                let task = dag
                    .add_task(task_fn::<(i32, i32), _, _>(|(a, b): (&i32, &i32)| a + b))
                    .depends_on((&chunk[0], &chunk[1]));
                next_level.push(task);
            } else {
                next_level.push(chunk[0]);
            }
        }

        current_level = next_level;
    }

    let root = current_level[0];

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    // Sum of powers of 2 from 0 to 7: 1+2+4+8+16+32+64+128 = 255
    assert_eq!(dag.get(root)?, 255);

    Ok(())
}

#[tokio::test]
async fn test_n_ary_tree() -> DagResult<()> {
    let dag = DagRunner::new();

    // Build a 3-ary tree (each node has 3 children)
    fn build_tree(dag: &DagRunner, depth: usize, value: i32) -> dagx::TaskHandle<i32> {
        if depth == 0 {
            let task = dag.add_task(task_fn::<(), _, _>(move |_: ()| value));
            return task.into();
        }

        let children: Vec<_> = (0..3)
            .map(|i| build_tree(dag, depth - 1, value * 3 + i))
            .collect();

        dag.add_task(task_fn::<(i32, i32, i32), _, _>(
            move |(a, b, c): (&i32, &i32, &i32)| value + a + b + c,
        ))
        .depends_on((&children[0], &children[1], &children[2]))
    }

    let root = build_tree(&dag, 3, 1);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(root)?;
    assert!(result > 0);

    Ok(())
}

#[tokio::test]
async fn test_unbalanced_tree() -> DagResult<()> {
    let dag = DagRunner::new();

    // Create an unbalanced tree
    //        root
    //       /    \
    //      a      b
    //     / \      \
    //    c   d      e
    //   /            \
    //  f              g

    let f: TaskHandle<_> = dag.add_task(task_fn::<(), _, _>(|_: ()| 1)).into();
    let g = dag.add_task(task_fn::<(), _, _>(|_: ()| 2));

    let c = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 2))
        .depends_on(f);

    let d_builder = dag.add_task(task_fn::<(), _, _>(|_: ()| 3));
    let d: dagx::TaskHandle<i32> = d_builder.into();

    let e = dag
        .add_task(task_fn::<i32, _, _>(|&x: &i32| x * 3))
        .depends_on(g);

    let a = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(c, d): (&i32, &i32)| c + d))
        .depends_on((&c, &d));

    let b = dag
        .add_task(task_fn::<i32, _, _>(|e: &i32| e + 10))
        .depends_on(e);

    let root = dag
        .add_task(task_fn::<(i32, i32), _, _>(|(a, b): (&i32, &i32)| a * b))
        .depends_on((&a, &b));

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(f)?, 1);
    assert_eq!(dag.get(c)?, 2);
    assert_eq!(dag.get(a)?, 5); // 2 + 3
    assert_eq!(dag.get(b)?, 16); // 6 + 10
    assert_eq!(dag.get(root)?, 80); // 5 * 16

    Ok(())
}

#[tokio::test]
async fn test_trie_like_structure() -> DagResult<()> {
    let dag = DagRunner::new();

    // Build a trie-like structure for paths
    let root: TaskHandle<_> = dag
        .add_task(task_fn::<(), _, _>(|_: ()| "root".to_string()))
        .into();

    // First level
    let usr = dag
        .add_task(task_fn::<String, _, _>(|r: &String| format!("{}/usr", r)))
        .depends_on(root);

    let etc = dag
        .add_task(task_fn::<String, _, _>(|r: &String| format!("{}/etc", r)))
        .depends_on(root);

    let var = dag
        .add_task(task_fn::<String, _, _>(|r: &String| format!("{}/var", r)))
        .depends_on(root);

    // Second level
    let usr_bin = dag
        .add_task(task_fn::<String, _, _>(|u: &String| format!("{}/bin", u)))
        .depends_on(usr);

    let usr_lib = dag
        .add_task(task_fn::<String, _, _>(|u: &String| format!("{}/lib", u)))
        .depends_on(usr);

    let etc_config = dag
        .add_task(task_fn::<String, _, _>(|e: &String| {
            format!("{}/config", e)
        }))
        .depends_on(etc);

    let var_log = dag
        .add_task(task_fn::<String, _, _>(|v: &String| format!("{}/log", v)))
        .depends_on(var);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    assert_eq!(dag.get(usr_bin)?, "root/usr/bin");
    assert_eq!(dag.get(usr_lib)?, "root/usr/lib");
    assert_eq!(dag.get(etc_config)?, "root/etc/config");
    assert_eq!(dag.get(var_log)?, "root/var/log");

    Ok(())
}

#[tokio::test]
async fn test_balanced_k_ary_tree() -> DagResult<()> {
    let dag = DagRunner::new();

    // Create a perfectly balanced 4-ary tree of depth 3
    const K: usize = 4;
    const DEPTH: usize = 3;

    fn create_balanced_tree(
        dag: &DagRunner,
        depth: usize,
        k: usize,
        node_id: usize,
    ) -> dagx::TaskHandle<usize> {
        if depth == 0 {
            let task = dag.add_task(task_fn::<(), _, _>(move |_: ()| node_id));
            return task.into();
        }

        let children: Vec<_> = (0..k)
            .map(|i| create_balanced_tree(dag, depth - 1, k, node_id * k + i))
            .collect();

        match k {
            2 => dag
                .add_task(task_fn::<(usize, usize), _, _>(
                    move |(a, b): (&usize, &usize)| node_id + a + b,
                ))
                .depends_on((&children[0], &children[1])),
            3 => dag
                .add_task(task_fn::<(usize, usize, usize), _, _>(
                    move |(a, b, c): (&usize, &usize, &usize)| node_id + a + b + c,
                ))
                .depends_on((&children[0], &children[1], &children[2])),
            4 => dag
                .add_task(task_fn::<(usize, usize, usize, usize), _, _>(
                    move |(a, b, c, d): (&usize, &usize, &usize, &usize)| node_id + a + b + c + d,
                ))
                .depends_on((&children[0], &children[1], &children[2], &children[3])),
            _ => panic!("Unsupported k value"),
        }
    }

    let root = create_balanced_tree(&dag, DEPTH, K, 0);

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(root)?;
    assert!(result > 0);

    Ok(())
}

#[tokio::test]
async fn test_merkle_tree_pattern() -> DagResult<()> {
    let dag = DagRunner::new();

    // Simulate a Merkle tree for 8 data blocks
    let data_blocks: Vec<_> = (0..8)
        .map(|i| {
            dag.add_task(task_fn::<(), _, _>(move |_: ()| {
                // Simulate hash of data block
                format!("hash_{}", i)
            }))
        })
        .collect();

    // Convert to TaskHandles
    let mut current_level: Vec<dagx::TaskHandle<String>> =
        data_blocks.into_iter().map(|t| t.into()).collect();

    while current_level.len() > 1 {
        let mut next_level = Vec::new();

        for pair in current_level.chunks(2) {
            if pair.len() == 2 {
                let hash_task = dag
                    .add_task(task_fn::<(String, String), _, _>(
                        |(left, right): (&String, &String)| format!("hash({}_{})", left, right),
                    ))
                    .depends_on((&pair[0], &pair[1]));
                next_level.push(hash_task);
            } else {
                next_level.push(pair[0]);
            }
        }

        current_level = next_level;
    }

    let root_hash = current_level[0];

    dag.run(|fut| tokio::spawn(fut).map(Result::unwrap)).await?;

    let result = dag.get(root_hash)?;
    assert!(result.starts_with("hash"));

    Ok(())
}
