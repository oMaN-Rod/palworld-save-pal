mod support;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use psp_core::progress::ProgressSink;
use psp_plugin::manifest::Capability;
use psp_plugin::status::RunStatus;

#[test]
fn log_lines_are_captured_at_their_level() {
    let mut h = support::harness(&[Capability::Log]);
    let (status, _) = h.run("log.info('one') log.warn('two') log.error('three') return ''");
    assert_eq!(status, RunStatus::Ok);
    let log = h.log();
    assert_eq!(log.len(), 3);
    assert_eq!(log[0].message, "one");
    assert_eq!(log[2].message, "three");
}

#[test]
fn the_log_global_is_absent_without_its_capability() {
    let mut h = support::harness(&[]);
    let (status, value) = h.run("return type(log)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("nil"));
}

#[test]
fn progress_is_available_without_any_capability() {
    let mut h = support::harness(&[]);
    let (status, value) = h.run("progress.report('working', 0.5) return type(progress.report)");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("function"));
}

#[test]
fn a_progress_fraction_outside_zero_to_one_is_refused() {
    let mut h = support::harness(&[]);
    let (status, value) = h.run("return tostring(pcall(progress.report, 'x', 2.0))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn storage_reads_what_the_host_supplied_and_buffers_what_it_writes() {
    let mut h = support::harness(&[Capability::Storage]);
    h.seed_storage("seen", "yes");
    let (status, value) = h.run(
        "local before = storage.get('seen')
         storage.set('count', '7')
         return tostring(before) .. ',' .. tostring(storage.get('nothing'))",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("yes,nil"));
    assert_eq!(h.storage_writes(), &[("count".to_string(), "7".to_string())]);
}

#[test]
fn ui_confirm_returns_false_when_no_dialog_is_attached() {
    let mut h = support::harness(&[Capability::UiDialog]);
    let (status, value) = h.run("return tostring(ui.confirm('really?'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn ui_confirm_returns_true_under_dry_run_without_asking() {
    let mut h = support::harness_dry(&[Capability::UiDialog]);
    let (status, value) = h.run("return tostring(ui.confirm('really?'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn ctx_exposes_the_run_shape() {
    let mut h = support::harness(&[]);
    let (status, value) = h.run(
        "return tostring(ctx.dry_run) .. ',' .. tostring(ctx.api_version) .. ',' .. type(ctx.args)",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false,1,table"));
}

#[test]
fn ui_confirm_reaches_a_supplied_dialog_that_answers_true() {
    let mut h = support::harness(&[Capability::UiDialog]).with_confirm(|_| true);
    let (status, value) = h.run("return tostring(ui.confirm('really?'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
}

#[test]
fn ui_confirm_reaches_a_supplied_dialog_that_answers_false() {
    let mut h = support::harness(&[Capability::UiDialog]).with_confirm(|_| false);
    let (status, value) = h.run("return tostring(ui.confirm('really?'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("false"));
}

#[test]
fn ui_confirm_dry_run_short_circuits_without_calling_the_dialog() {
    let called = Rc::new(Cell::new(false));
    let called_from_dialog = called.clone();
    let mut h = support::harness_dry(&[Capability::UiDialog]).with_confirm(move |_| {
        called_from_dialog.set(true);
        true
    });
    let (status, value) = h.run("return tostring(ui.confirm('really?'))");
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("true"));
    assert!(!called.get(), "a dry run must not call the dialog at all");
}

#[test]
fn progress_report_reaches_a_supplied_sink_with_the_message_intact() {
    let calls: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let calls_from_sink = calls.clone();
    let sink: ProgressSink = Arc::new(move |text: &str| {
        calls_from_sink.lock().expect("the mutex is never poisoned").push(text.to_string());
    });
    let mut h = support::harness(&[]).with_progress(sink);
    let (status, _) = h.run("progress.report('halfway there', 0.5) return ''");
    assert_eq!(status, RunStatus::Ok);
    let recorded = calls.lock().expect("the mutex is never poisoned");
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].contains("halfway there"), "got {:?}", recorded[0]);
}

#[test]
fn every_service_survives_hostile_arguments() {
    let mut h = support::harness(&[
        Capability::Log, Capability::Storage, Capability::UiDialog,
    ]);
    let (status, value) = h.run(
        "local vals = { nil, true, 0, -1, 1/0, 0/0, '', 'x', {}, print }
         local fns = { log.info, log.warn, log.error, progress.report,
                       ui.confirm, storage.get, storage.set }
         for _, fn in pairs(fns) do
           for i = 1, 10 do for j = 1, 10 do pcall(fn, vals[i], vals[j]) end end
         end
         return 'survived'",
    );
    assert_eq!(status, RunStatus::Ok);
    assert_eq!(value.as_deref(), Some("survived"));
}
