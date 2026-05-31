//! End-to-end behavior tests for altair-concurrent.

use altair_concurrent::prelude::*;
use pretty_assertions::assert_eq;
use std::time::Duration;

#[tokio::test]
async fn three_parallel_tasks_share_results() {
    let tasks: TaskMap<String> = TaskMap::new()
        .insert("alpha", |_| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Ok::<_, std::io::Error>("a".to_string())
        })
        .insert("beta", |_| async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok::<_, std::io::Error>("b".to_string())
        })
        .insert("gamma", |_| async {
            Ok::<_, std::io::Error>("g".to_string())
        });

    let results = execute_concurrently(tasks).await.unwrap();
    assert_eq!(results.len(), 3);
    assert_eq!(results["alpha"], "a");
    assert_eq!(results["beta"], "b");
    assert_eq!(results["gamma"], "g");
}

#[tokio::test]
async fn panicking_task_surfaces_as_join_error() {
    let tasks: TaskMap<u32> = TaskMap::new().insert("boom", |_| async {
        panic!("intentional panic");
        #[allow(unreachable_code)]
        Ok::<_, std::io::Error>(0)
    });

    let err = execute_concurrently(tasks).await.unwrap_err();
    match err {
        Error::Join(je) => assert!(je.is_panic(), "expected panic, got {je:?}"),
        other => panic!("expected Error::Join with is_panic, got {other:?}"),
    }
}

#[tokio::test]
#[allow(clippy::items_after_statements, clippy::too_many_lines)]
async fn one_hundred_tasks_all_complete() {
    // Stress-test the JoinSet path with 100 short tasks.
    let mut map: TaskMap<u32> = TaskMap::new();
    // Use a static array of 100 hard-coded names so each task gets a
    // distinct &'static str without leaking memory.
    static NAMES: [&str; 100] = const {
        let mut a = [""; 100];
        a[0] = "task-0";
        a[1] = "task-1";
        a[2] = "task-2";
        a[3] = "task-3";
        a[4] = "task-4";
        a[5] = "task-5";
        a[6] = "task-6";
        a[7] = "task-7";
        a[8] = "task-8";
        a[9] = "task-9";
        a[10] = "task-10";
        a[11] = "task-11";
        a[12] = "task-12";
        a[13] = "task-13";
        a[14] = "task-14";
        a[15] = "task-15";
        a[16] = "task-16";
        a[17] = "task-17";
        a[18] = "task-18";
        a[19] = "task-19";
        a[20] = "task-20";
        a[21] = "task-21";
        a[22] = "task-22";
        a[23] = "task-23";
        a[24] = "task-24";
        a[25] = "task-25";
        a[26] = "task-26";
        a[27] = "task-27";
        a[28] = "task-28";
        a[29] = "task-29";
        a[30] = "task-30";
        a[31] = "task-31";
        a[32] = "task-32";
        a[33] = "task-33";
        a[34] = "task-34";
        a[35] = "task-35";
        a[36] = "task-36";
        a[37] = "task-37";
        a[38] = "task-38";
        a[39] = "task-39";
        a[40] = "task-40";
        a[41] = "task-41";
        a[42] = "task-42";
        a[43] = "task-43";
        a[44] = "task-44";
        a[45] = "task-45";
        a[46] = "task-46";
        a[47] = "task-47";
        a[48] = "task-48";
        a[49] = "task-49";
        a[50] = "task-50";
        a[51] = "task-51";
        a[52] = "task-52";
        a[53] = "task-53";
        a[54] = "task-54";
        a[55] = "task-55";
        a[56] = "task-56";
        a[57] = "task-57";
        a[58] = "task-58";
        a[59] = "task-59";
        a[60] = "task-60";
        a[61] = "task-61";
        a[62] = "task-62";
        a[63] = "task-63";
        a[64] = "task-64";
        a[65] = "task-65";
        a[66] = "task-66";
        a[67] = "task-67";
        a[68] = "task-68";
        a[69] = "task-69";
        a[70] = "task-70";
        a[71] = "task-71";
        a[72] = "task-72";
        a[73] = "task-73";
        a[74] = "task-74";
        a[75] = "task-75";
        a[76] = "task-76";
        a[77] = "task-77";
        a[78] = "task-78";
        a[79] = "task-79";
        a[80] = "task-80";
        a[81] = "task-81";
        a[82] = "task-82";
        a[83] = "task-83";
        a[84] = "task-84";
        a[85] = "task-85";
        a[86] = "task-86";
        a[87] = "task-87";
        a[88] = "task-88";
        a[89] = "task-89";
        a[90] = "task-90";
        a[91] = "task-91";
        a[92] = "task-92";
        a[93] = "task-93";
        a[94] = "task-94";
        a[95] = "task-95";
        a[96] = "task-96";
        a[97] = "task-97";
        a[98] = "task-98";
        a[99] = "task-99";
        a
    };
    for (i, name) in NAMES.iter().enumerate() {
        let value = u32::try_from(i).unwrap();
        map = map.insert(name, move |_| async move { Ok::<_, std::io::Error>(value) });
    }

    let results = execute_concurrently(map).await.unwrap();
    assert_eq!(results.len(), 100);
    // Spot-check a handful — full map equality would be noisy.
    assert_eq!(results["task-0"], 0);
    assert_eq!(results["task-50"], 50);
    assert_eq!(results["task-99"], 99);
}

#[tokio::test]
async fn duplicate_task_name_keeps_last_inserted() {
    // Last-write-wins: the second `insert("k", ...)` replaces the first.
    let tasks: TaskMap<u32> = TaskMap::new()
        .insert("k", |_| async { Ok::<_, std::io::Error>(1) })
        .insert("k", |_| async { Ok::<_, std::io::Error>(2) });
    assert_eq!(tasks.len(), 1);
    let results = execute_concurrently(tasks).await.unwrap();
    assert_eq!(results["k"], 2);
}

#[tokio::test]
async fn cancellation_token_propagates_to_tasks() {
    let token = CancellationToken::new();
    let m: TaskMap<bool> = TaskMap::new().insert("respect_ct", |ct| async move {
        tokio::select! {
            () = ct.cancelled() => Ok::<_, std::io::Error>(false),
            () = tokio::time::sleep(Duration::from_secs(10)) => Ok::<_, std::io::Error>(true),
        }
    });

    let inner = token.clone();
    let handle =
        tokio::spawn(async move { execute_concurrently(m).with_cancellation(inner).await });
    tokio::time::sleep(Duration::from_millis(20)).await;
    token.cancel();
    let result = handle.await.unwrap();
    // External cancel may yield either flavor; success means the task observed the token.
    assert!(result.is_err() || !result.unwrap()["respect_ct"]);
}
