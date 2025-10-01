//! Grid pattern DAG tests

use crate::common::task_fn;
use dagx::{DagResult, DagRunner};

#[tokio::test]
async fn test_2d_grid_pattern() -> DagResult<()> {
    let dag = DagRunner::new();

    // Create a 4x4 grid where each cell depends on the cell above and to the left
    let mut grid: Vec<Vec<Option<dagx::TaskHandle<i32>>>> = vec![vec![None; 4]; 4];

    // Initialize first row and column
    for i in 0..4 {
        let task = dag.add_task(task_fn(move |_: ()| async move { i as i32 }));
        grid[0][i] = Some((&task).into());
        if i > 0 {
            let task = dag.add_task(task_fn(move |_: ()| async move { i as i32 * 10 }));
            grid[i][0] = Some((&task).into());
        }
    }

    // Fill the grid with dependencies
    for i in 1..4 {
        for j in 1..4 {
            let above = grid[i - 1][j].as_ref().unwrap();
            let left = grid[i][j - 1].as_ref().unwrap();

            let task = dag
                .add_task(task_fn(move |(a, l): (i32, i32)| async move { a + l }))
                .depends_on((above, left));
            grid[i][j] = Some(task);
        }
    }

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Check corner values
    let bottom_right = dag.get(grid[3][3].as_ref().unwrap())?;
    assert!(bottom_right > 0);

    Ok(())
}

#[tokio::test]
async fn test_3d_grid_pattern() -> DagResult<()> {
    let dag = DagRunner::new();

    // Create a 3x3x3 grid
    let size = 3;
    let mut grid: Vec<Vec<Vec<Option<dagx::TaskHandle<usize>>>>> =
        vec![vec![vec![None; size]; size]; size];

    // Initialize origin
    let origin = dag.add_task(task_fn(|_: ()| async { 1 }));
    grid[0][0][0] = Some((&origin).into());

    // Fill the 3D grid
    for x in 0..size {
        for y in 0..size {
            for z in 0..size {
                if x == 0 && y == 0 && z == 0 {
                    continue;
                }

                let deps: Vec<dagx::TaskHandle<usize>> = vec![
                    if x > 0 { grid[x - 1][y][z] } else { None },
                    if y > 0 { grid[x][y - 1][z] } else { None },
                    if z > 0 { grid[x][y][z - 1] } else { None },
                ]
                .into_iter()
                .flatten()
                .collect();

                let task_handle: dagx::TaskHandle<usize> = match deps.len() {
                    0 => {
                        let t = dag.add_task(task_fn(move |_: ()| async move { x + y + z }));
                        (&t).into()
                    }
                    1 => dag
                        .add_task(task_fn(move |prev: usize| async move { prev + 1 }))
                        .depends_on(deps[0]),
                    2 => dag
                        .add_task(task_fn(move |(a, b): (usize, usize)| async move { a + b }))
                        .depends_on((&deps[0], &deps[1])),
                    3 => dag
                        .add_task(task_fn(
                            move |(a, b, c): (usize, usize, usize)| async move { a + b + c },
                        ))
                        .depends_on((&deps[0], &deps[1], &deps[2])),
                    _ => unreachable!(),
                };

                grid[x][y][z] = Some(task_handle);
            }
        }
    }

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Check that all cells have values
    for row in &grid {
        for col in row {
            for task in col.iter().flatten() {
                let _ = dag.get(task)?;
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_hexagonal_grid() -> DagResult<()> {
    let dag = DagRunner::new();

    // Hexagonal grid with axial coordinates
    let mut hex_grid = std::collections::HashMap::new();

    // Center hex
    let center_builder = dag.add_task(task_fn(|_: ()| async { 100 }));
    let center: dagx::TaskHandle<i32> = (&center_builder).into();
    hex_grid.insert((0, 0), center);

    // Ring 1 (6 hexes around center)
    let directions = [(1, 0), (0, 1), (-1, 1), (-1, 0), (0, -1), (1, -1)];

    for (i, &(dq, dr)) in directions.iter().enumerate() {
        let neighbor: dagx::TaskHandle<i32> = dag
            .add_task(task_fn(
                move |c: i32| async move { c + (i as i32 + 1) * 10 },
            ))
            .depends_on(center);
        hex_grid.insert((dq, dr), neighbor);
    }

    // Ring 2 (12 hexes)
    for &(q, r) in &[
        (2, 0),
        (1, 1),
        (0, 2),
        (-1, 2),
        (-2, 2),
        (-2, 1),
        (-2, 0),
        (-1, -1),
        (0, -2),
        (1, -2),
        (2, -2),
        (2, -1),
    ] {
        // Find neighbors that exist
        let neighbors: Vec<_> = directions
            .iter()
            .filter_map(|&(dq, dr)| hex_grid.get(&(q - dq, r - dr)).cloned())
            .take(2) // Just use first 2 neighbors for simplicity
            .collect();

        let task: dagx::TaskHandle<i32> = if neighbors.len() == 2 {
            dag.add_task(task_fn(move |(a, b): (i32, i32)| async move {
                (a + b) / 2 + q + r
            }))
            .depends_on((&neighbors[0], &neighbors[1]))
        } else if neighbors.len() == 1 {
            dag.add_task(task_fn(move |n: i32| async move { n + q + r }))
                .depends_on(neighbors[0])
        } else {
            let t = dag.add_task(task_fn(move |_: ()| async move { q + r }));
            (&t).into()
        };

        hex_grid.insert((q, r), task);
    }

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify center and some ring values
    assert_eq!(dag.get(hex_grid.get(&(0, 0)).unwrap())?, 100);

    Ok(())
}

#[tokio::test]
async fn test_toroidal_grid() -> DagResult<()> {
    let dag = DagRunner::new();

    const SIZE: usize = 4;
    let mut grid: Vec<Vec<Option<dagx::TaskHandle<i32>>>> = vec![vec![None; SIZE]; SIZE];

    // Initialize all cells
    #[allow(clippy::needless_range_loop)]
    for i in 0..SIZE {
        for j in 0..SIZE {
            let task = dag.add_task(task_fn(move |_: ()| async move { (i * SIZE + j) as i32 }));
            grid[i][j] = Some((&task).into());
        }
    }

    // Create toroidal connections (wrapping edges)
    let mut torus_tasks = Vec::new();

    #[allow(clippy::needless_range_loop)]
    for i in 0..SIZE {
        for j in 0..SIZE {
            let curr = grid[i][j].as_ref().unwrap();
            let up = grid[(i + SIZE - 1) % SIZE][j].as_ref().unwrap();
            let down = grid[(i + 1) % SIZE][j].as_ref().unwrap();
            let left = grid[i][(j + SIZE - 1) % SIZE].as_ref().unwrap();
            let right = grid[i][(j + 1) % SIZE].as_ref().unwrap();

            let task = dag
                .add_task(task_fn(
                    move |(c, u, d, l, r): (i32, i32, i32, i32, i32)| async move {
                        // Average of self and 4 neighbors
                        (c + u + d + l + r) / 5
                    },
                ))
                .depends_on((curr, up, down, left, right));

            torus_tasks.push(task);
        }
    }

    dag.run(|fut| {
        tokio::spawn(fut);
    })
    .await?;

    // Verify all torus tasks completed
    for task in torus_tasks {
        let _ = dag.get(task)?;
    }

    Ok(())
}
