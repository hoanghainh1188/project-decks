//! The finish forecast (phase 18 P4, step 4b): Monte Carlo over the effort ranges, on the
//! project leveled once with each person's order kept, BetaPERT samples and a Gaussian
//! copula for correlation. A read: nothing goes into the command log.

use std::sync::atomic::AtomicBool;

use chrono::Datelike;

use sheafly_engine::command::Command;
use sheafly_engine::fields::{FieldKind, FieldValue};
use sheafly_engine::forecast::{forecast, forecast_with_threads, Forecast, TaskForecast};
use sheafly_engine::model::{ForecastSettings, ResourceKind, Strength, Task};
use sheafly_engine::timeutil::timestamp;
use sheafly_engine::{Document, EngineError, Project};

const DAY: i64 = 480;
const RUNS: u32 = 400;

/// 2026-01-05 is a Monday; `day` 0 is that Monday.
fn at(day: u32, hour: u32) -> i64 {
    let date = chrono::NaiveDate::from_ymd_opt(2026, 1, 5)
        .unwrap()
        .checked_add_signed(chrono::Duration::days(i64::from(day)))
        .unwrap();
    timestamp(date.year(), date.month(), date.day(), hour, 0)
}

/// `count` one-day tasks from Monday, each on its own person, none linked.
fn doc(count: u32) -> Document {
    let mut project = Project::new("Dự báo", at(0, 8));
    for uid in 1..=count {
        let mut task = Task::new(uid, format!("Việc {uid}"));
        task.duration = DAY;
        task.remaining_duration = DAY;
        task.work = DAY;
        task.remaining_work = DAY;
        project.tasks.push(task);
    }
    project.next_task_uid = count + 1;
    let mut doc = Document::from_project(project);
    for uid in 1..=count {
        apply(&mut doc, Command::AddResource { name: format!("Người {uid}"), kind: ResourceKind::Person });
        apply(&mut doc, Command::AssignResource { task_uid: uid, resource_uid: uid, units: 1.0 });
    }
    doc
}

fn apply(doc: &mut Document, command: Command) {
    doc.apply(command).unwrap();
}

fn range(doc: &mut Document, uid: u32, low: i64, high: i64) {
    apply(doc, Command::SetWorkRange { uid, low: Some(low), high: Some(high) });
}

fn link(doc: &mut Document, from: u32, to: u32) {
    apply(
        doc,
        Command::AddDependency { predecessor_uid: from, successor_uid: to, kind: sheafly_engine::model::DepKind::Fs, lag: 0 },
    );
}

fn settings(doc: &mut Document, field: Option<u32>, group: Strength, common: Strength) {
    apply(doc, Command::SetForecastSettings { settings: ForecastSettings { field, group, common } });
}

fn run(doc: &Document) -> Forecast {
    forecast(&doc.project, RUNS, &AtomicBool::new(false)).unwrap()
}

fn of(result: &Forecast, uid: u32) -> &TaskForecast {
    result.tasks.iter().find(|task| task.uid == uid).unwrap()
}

fn finish(doc: &Document, uid: u32) -> i64 {
    doc.project.task(uid).unwrap().finish
}

#[test]
fn without_ranges_every_percentile_is_the_scheduled_date() {
    let mut doc = doc(2);
    link(&mut doc, 1, 2);
    apply(&mut doc, Command::SetDeadline { uid: 2, date: Some(at(1, 17)) });
    let result = run(&doc);
    assert_eq!(result.iterations, RUNS);
    let task = of(&result, 2);
    assert_eq!((task.p50, task.p80, task.p90), (finish(&doc, 2), finish(&doc, 2), finish(&doc, 2)));
    assert_eq!(task.on_time, Some(1.0));
    assert_eq!(task.criticality, 1.0);
    assert_eq!((result.finish.p50, result.finish.p90), (finish(&doc, 2), finish(&doc, 2)));
    assert_eq!(result.planned_finish, finish(&doc, 2));
    assert_eq!(result.leveled_finish, finish(&doc, 2));
    assert_eq!(result.histogram, vec![(at(1, 0), RUNS)]);
}

#[test]
fn the_same_project_gives_the_same_numbers_on_any_number_of_threads() {
    let mut doc = doc(6);
    for uid in 1..=6 {
        range(&mut doc, uid, DAY / 2, 3 * DAY);
    }
    link(&mut doc, 1, 2);
    link(&mut doc, 3, 4);
    let one = forecast_with_threads(&doc.project, RUNS, 1, &AtomicBool::new(false)).unwrap();
    let many = forecast_with_threads(&doc.project, RUNS, 5, &AtomicBool::new(false)).unwrap();
    assert_eq!(one, many);
    assert_eq!(one, run(&doc));
}

#[test]
fn branches_that_merge_finish_later_than_their_middle_estimates() {
    // Two parallel one-day tasks, each as likely to be short as long, then a milestone.
    let mut doc = doc(2);
    apply(&mut doc, Command::AddMilestone { name: "Xong".into(), after_uid: Some(2) });
    let milestone = doc.project.tasks.last().unwrap().uid;
    link(&mut doc, 1, milestone);
    link(&mut doc, 2, milestone);
    range(&mut doc, 1, DAY / 2, 3 * DAY / 2);
    range(&mut doc, 2, DAY / 2, 3 * DAY / 2);
    let result = run(&doc);
    // Each task alone is as likely early as late; the later of the two is not.
    assert!(of(&result, milestone).p50 > finish(&doc, milestone), "{:?}", of(&result, milestone));
    assert!(of(&result, 1).p50 <= finish(&doc, 1) + 3_600_000);
    assert!(result.finish.p50 <= result.finish.p80 && result.finish.p80 <= result.finish.p90);
}

#[test]
fn tasks_that_move_together_spread_the_finish_wider() {
    let spread = |group: Strength, common: Strength| {
        let mut doc = doc(10);
        apply(&mut doc, Command::AddField { name: "Mảng".into(), kind: FieldKind::Text });
        for uid in 1..=10 {
            range(&mut doc, uid, DAY / 2, 2 * DAY);
            apply(&mut doc, Command::SetFieldValue { uid, field_id: 1, value: Some(FieldValue::Text("A".into())) });
        }
        // A chain, so the finish adds the tasks up.
        for uid in 1..10 {
            link(&mut doc, uid, uid + 1);
        }
        settings(&mut doc, Some(1), group, common);
        let result = run(&doc);
        result.finish.p90 - result.finish.p50
    };
    let alone = spread(Strength::None, Strength::None);
    let together = spread(Strength::High, Strength::None);
    let common = spread(Strength::None, Strength::High);
    assert!(together > alone, "{together} vs {alone}");
    assert!(common > alone, "{common} vs {alone}");
}

#[test]
fn finished_work_stays_and_started_work_varies_only_in_what_is_left() {
    let mut doc = doc(2);
    apply(&mut doc, Command::SetStatusDate { date: Some(at(0, 17)) });
    range(&mut doc, 1, DAY / 2, 4 * DAY);
    range(&mut doc, 2, DAY / 2, 4 * DAY);
    apply(&mut doc, Command::SetPercentComplete { uid: 1, percent: 100.0 });
    apply(&mut doc, Command::SetPercentComplete { uid: 2, percent: 50.0 });
    let result = run(&doc);
    let done = of(&result, 1);
    assert_eq!((done.p50, done.p90), (finish(&doc, 1), finish(&doc, 1)));
    // Half a day is done; the rest varies, so it can finish no earlier than the done part.
    let started = of(&result, 2);
    assert!(started.p90 > started.p50);
    assert!(started.p50 > doc.project.task(2).unwrap().start + 4 * 3_600_000);
}

#[test]
fn one_persons_tasks_keep_the_order_leveling_gives_them() {
    // Two unlinked tasks on one person: leveling puts them one after the other.
    let mut doc = doc(2);
    let second = doc.project.assignments[1].uid;
    apply(&mut doc, Command::RemoveAssignment { uid: second });
    apply(&mut doc, Command::AssignResource { task_uid: 2, resource_uid: 1, units: 1.0 });
    range(&mut doc, 1, DAY / 2, 3 * DAY);
    let result = run(&doc);
    // Without ranges on task 2 its finish moves only because task 1 before it does.
    let first = of(&result, 1);
    let second = of(&result, 2);
    assert!(second.p50 > first.p50 && second.p90 > first.p90);
    assert!(result.finish.p50 >= at(1, 12), "the two days of work are in sequence");
    // As scheduled, the two overlap; leveled, the second follows the first.
    assert_eq!(result.planned_finish, at(0, 17));
    assert_eq!(result.leveled_finish, at(1, 17));
}

#[test]
fn ranges_on_summaries_and_milestones_are_left_out() {
    let mut doc = doc(2);
    range(&mut doc, 1, DAY / 2, 3 * DAY);
    // Task 1 becomes a summary of task 2; its range stays in the data but means nothing.
    apply(&mut doc, Command::Indent { uid: 2 });
    let result = run(&doc);
    assert_eq!(result.finish.p90, result.finish.p50);
}

#[test]
fn a_cancelled_forecast_stops() {
    let mut doc = doc(3);
    range(&mut doc, 1, DAY / 2, 3 * DAY);
    let cancel = AtomicBool::new(true);
    assert!(matches!(forecast(&doc.project, RUNS, &cancel), Err(EngineError::Cancelled)));
}

#[test]
fn five_hundred_people_and_five_thousand_tasks_is_quick() {
    let mut project = Project::new("Lớn", at(0, 8));
    for uid in 1..=5000u32 {
        let mut task = Task::new(uid, format!("Việc {uid}"));
        let work = 60 * i64::from(1 + uid % 16);
        task.duration = work;
        task.remaining_duration = work;
        task.work = work;
        task.remaining_work = work;
        task.work_low = Some(work / 2);
        task.work_high = Some(work * 2);
        project.tasks.push(task);
    }
    project.next_task_uid = 5001;
    let started = std::time::Instant::now();
    let result = forecast(&project, 50, &AtomicBool::new(false)).unwrap();
    assert_eq!(result.tasks.len(), 5000);
    // Debug builds are slow; the bench in release is far below this.
    assert!(started.elapsed().as_secs() < 60);
}

/// Three one-day tasks on person 1 (each `units` of them), the rest of `doc(3)` removed.
fn one_person(units: f64) -> Document {
    let mut doc = doc(3);
    let others: Vec<u32> = doc.project.assignments.iter().map(|a| a.uid).collect();
    for uid in others {
        apply(&mut doc, Command::RemoveAssignment { uid });
    }
    for task in 1..=3 {
        apply(&mut doc, Command::AssignResource { task_uid: task, resource_uid: 1, units });
    }
    doc
}

#[test]
fn a_project_already_leveled_forecasts_its_own_dates() {
    let mut doc = one_person(1.0);
    apply(&mut doc, Command::LevelResources);
    let last = (1..=3).map(|uid| finish(&doc, uid)).max().unwrap();
    let result = run(&doc);
    assert_eq!(result.planned_finish, last);
    assert_eq!(result.leveled_finish, last, "leveling's delays are not counted twice");
    assert_eq!(result.finish.p90, last);
}

#[test]
fn part_time_work_side_by_side_keeps_what_leveling_put_after_it() {
    // A lasts two days at half time and B one day at half time, side by side; C, also at
    // half time, fits beside B but must wait for A.
    let mut doc = one_person(0.5);
    apply(&mut doc, Command::SetDuration { uid: 1, minutes: 4 * DAY });
    apply(&mut doc, Command::LevelResources);
    let leveled: Vec<i64> = (1..=3).map(|uid| finish(&doc, uid)).collect();
    let result = run(&doc);
    for (uid, finish) in (1..=3).zip(leveled) {
        assert_eq!(of(&result, uid).p50, finish, "task {uid}");
    }
}

#[test]
fn a_range_scales_from_the_effort_the_task_has_now() {
    // Today's effort is two days, but the range says half a day to one day.
    let mut doc = doc(1);
    apply(&mut doc, Command::SetWork { uid: 1, minutes: 2 * DAY });
    range(&mut doc, 1, DAY / 2, DAY);
    let result = run(&doc);
    let start = doc.project.task(1).unwrap().start;
    assert!(of(&result, 1).p90 <= start + 24 * 3_600_000, "{:?}", of(&result, 1));
}

#[test]
fn a_backwards_range_from_a_file_is_left_out_rather_than_failing() {
    let mut doc = doc(1);
    let mut project = doc.project.clone();
    project.tasks[0].work_low = Some(2 * DAY);
    project.tasks[0].work_high = Some(DAY);
    let result = forecast(&project, RUNS, &AtomicBool::new(false)).unwrap();
    assert_eq!(result.finish.p50, result.finish.p90);
    range(&mut doc, 1, DAY / 2, 2 * DAY);
}

#[test]
fn a_fixed_duration_task_with_people_does_not_vary() {
    let mut doc = doc(1);
    apply(&mut doc, Command::SetTaskType { uid: 1, task_type: sheafly_engine::model::TaskType::FixedDuration, effort_driven: false });
    range(&mut doc, 1, DAY / 2, 3 * DAY);
    let result = run(&doc);
    assert_eq!(result.finish.p50, result.finish.p90);
}

#[test]
fn a_split_task_keeps_its_pause_inside_the_work() {
    let mut doc = doc(1);
    apply(&mut doc, Command::SetDuration { uid: 1, minutes: 2 * DAY });
    apply(&mut doc, Command::SplitTask { uid: 1, at: at(1, 8), gap: DAY });
    range(&mut doc, 1, DAY, 4 * DAY);
    let result = run(&doc);
    let start = doc.project.task(1).unwrap().start;
    // At least half the shortest work, a day's pause, then the rest.
    assert!(of(&result, 1).p50 > start + 2 * 24 * 3_600_000);
    assert!(result.finish.p90 > result.finish.p50);
}

#[test]
fn runs_are_capped_and_at_least_one() {
    let doc = doc(1);
    assert_eq!(forecast(&doc.project, 0, &AtomicBool::new(false)).unwrap().iterations, 1);
    let many = forecast(&doc.project, 1_000_000, &AtomicBool::new(false)).unwrap();
    assert_eq!(many.iterations, sheafly_engine::forecast::MAX_ITERATIONS);
}
