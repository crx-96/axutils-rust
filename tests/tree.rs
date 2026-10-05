use axutils::tree::{self, TreeBuildError, TreeNode};

#[derive(Clone, Debug, Eq, PartialEq)]
struct Record {
    id: u32,
    parent: Option<u32>,
    order: i16,
}

fn record(id: u32, parent: Option<u32>, order: i16) -> Record {
    Record { id, parent, order }
}

fn forest(records: Vec<Record>, depth: usize) -> Result<Vec<TreeNode<Record>>, TreeBuildError> {
    tree::build_forest(
        records,
        depth,
        |row| row.id,
        |row| row.parent,
        |row| row.order,
    )
}

#[test]
fn roots_and_children_sort_by_order_then_id_independent_of_input() {
    let records = vec![
        record(30, Some(10), 2),
        record(20, None, 0),
        record(40, Some(10), -1),
        record(11, Some(10), 2),
        record(10, None, 0),
        record(50, None, -1),
    ];
    let expected = forest(records.clone(), 2).unwrap();
    let reversed = forest(records.into_iter().rev().collect(), 2).unwrap();
    assert_eq!(expected, reversed);
    assert_eq!(
        expected.iter().map(|node| node.item.id).collect::<Vec<_>>(),
        [50, 10, 20]
    );
    assert_eq!(
        expected[1]
            .children
            .iter()
            .map(|node| node.item.id)
            .collect::<Vec<_>>(),
        [40, 11, 30]
    );
}

#[test]
fn empty_input_accepts_zero_depth_without_reading_metadata() {
    let result = tree::build_forest::<(), u32, i16>(
        vec![],
        0,
        |_| panic!("不应提取空输入"),
        |_| panic!("不应提取空输入"),
        |_| panic!("不应提取空输入"),
    );
    assert_eq!(result.unwrap(), []);
    assert!(forest(vec![], usize::MAX).unwrap().is_empty());
}

#[test]
fn nonempty_input_rejects_zero_depth_before_reading_metadata() {
    let result = tree::build_forest::<(), u32, i16>(
        vec![()],
        0,
        |_| panic!("零深度应先拒绝"),
        |_| panic!("零深度应先拒绝"),
        |_| panic!("零深度应先拒绝"),
    );
    assert_eq!(result.unwrap_err(), TreeBuildError::DepthExceeded);
}

#[test]
fn rejects_duplicate_ids_even_when_relationships_differ() {
    assert_eq!(
        forest(vec![record(1, None, 0), record(1, Some(1), 1)], 8).unwrap_err(),
        TreeBuildError::DuplicateId
    );
}

#[test]
fn rejects_missing_parent_even_beside_valid_roots() {
    assert_eq!(
        forest(vec![record(1, None, 0), record(2, Some(3), 0)], 8).unwrap_err(),
        TreeBuildError::MissingParent
    );
}

#[test]
fn rejects_self_loop_and_independent_cycles_without_dropping_nodes() {
    for records in [
        vec![record(1, Some(1), 0)],
        vec![record(1, Some(2), 0), record(2, Some(1), 0)],
        vec![
            record(1, None, 0),
            record(2, Some(3), 0),
            record(3, Some(2), 0),
            record(4, Some(3), 0),
        ],
    ] {
        assert_eq!(forest(records, 8).unwrap_err(), TreeBuildError::Cycle);
    }
}

#[test]
fn root_depth_is_one_and_depth_limit_is_inclusive() {
    assert!(forest(vec![record(1, None, 0)], 1).is_ok());
    let records = vec![
        record(1, None, 0),
        record(2, Some(1), 0),
        record(3, Some(2), 0),
    ];
    assert_eq!(
        forest(records.clone(), 2).unwrap_err(),
        TreeBuildError::DepthExceeded
    );
    assert!(forest(records.clone(), 3).is_ok());
    assert!(forest(records, usize::MAX).is_ok());
}

#[test]
fn identifiers_and_sort_values_need_no_clone_or_copy() {
    #[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
    struct Id(String);
    #[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
    struct Order(i16);

    let mut id_calls = 0;
    let mut parent_calls = 0;
    let mut order_calls = 0;
    let records = vec![("child", Some("root")), ("root", None)];
    let result = tree::build_forest(
        records,
        2,
        |row| {
            id_calls += 1;
            Id(row.0.to_owned())
        },
        |row| {
            parent_calls += 1;
            row.1.map(|value| Id(value.to_owned()))
        },
        |_| {
            order_calls += 1;
            Order(0)
        },
    )
    .unwrap();
    assert_eq!(result[0].item.0, "root");
    assert_eq!(result[0].children[0].item.0, "child");
    assert_eq!((id_calls, parent_calls, order_calls), (2, 2, 2));
}

#[test]
fn map_preserves_sibling_order_and_calls_parent_after_children() {
    let root = forest(
        vec![
            record(1, None, 0),
            record(3, Some(1), 0),
            record(2, Some(1), 0),
            record(4, Some(2), 0),
        ],
        3,
    )
    .unwrap()
    .pop()
    .unwrap();
    let mut visited = Vec::new();
    let converted = root
        .try_map(|row, children: Vec<String>| {
            visited.push(row.id);
            Ok::<_, ()>(format!("{}[{}]", row.id, children.join(",")))
        })
        .unwrap();
    assert_eq!(visited, [4, 2, 3, 1]);
    assert_eq!(converted, "1[2[4[]],3[]]");
}

#[test]
fn map_propagates_first_error_and_stops_before_later_siblings_and_parent() {
    let root = forest(
        vec![
            record(1, None, 0),
            record(2, Some(1), 0),
            record(3, Some(1), 0),
        ],
        2,
    )
    .unwrap()
    .pop()
    .unwrap();
    let mut visited = Vec::new();
    let expected = String::from("调用方的转换失败");
    let converted = root.try_map(|row, _: Vec<u32>| {
        visited.push(row.id);
        if row.id == 2 {
            Err(expected.clone())
        } else {
            Ok(row.id)
        }
    });
    assert_eq!(converted, Err(expected));
    assert_eq!(visited, [2]);
}

#[test]
fn deep_chain_builds_and_converts_with_explicit_stacks() {
    const DEPTH: u32 = 4_096;
    let records = (1..=DEPTH)
        .rev()
        .map(|id| record(id, (id > 1).then_some(id - 1), 0))
        .collect();
    let root = forest(records, DEPTH as usize).unwrap().pop().unwrap();
    let count = root
        .try_map(|_, children: Vec<usize>| Ok::<_, ()>(1 + children.into_iter().sum::<usize>()))
        .unwrap();
    assert_eq!(count, DEPTH as usize);
}
