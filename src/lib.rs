use minitrace::{
    collector::{SpanContext, SpanId, TraceId},
    future::FutureExt,
    local::{LocalCollector, LocalSpan},
    trace, Span,
};
use wasm_bindgen::prelude::*;
use worker::*;

// This is a simple reproduction for a problem I'm facing with minitrace.
//
// The main issue here is the inconsistency behaviour of Span and LocalSpan.
// I'm able to use LocalSpan for most cases, but when I need to use it with in_span, it doesn't work as expected.
// As a workaround, I'm either creating and entering the LocalSpan manually, or use `#[trace]` that handles that in a nice way.
// Ideally, I want to be able to use LocalSpan with in_span, and have it working just like the regular Span.
//
// The workaround seems to work pretty well, but it introduces another issue: I can't create a LocalSpan with a parent LocalSpan manually.
//
// The LocalSpan does not have `with_parent` method, and I can't use `in_span` with it.
// So to achieve hirearchy, I must split my code to smaller functions and use `#[trace]`.

// This works as expected, the function is being traced and included in SpanRecords
// I don't need to deal with picking LocalSpan/Span and it's being picked and handlded automatically.
#[trace]
async fn func_with_trace() {
    // This one is created inside this span as LocalSpan and actually works fine.
    let _child = LocalSpan::enter_with_local_parent("child");

    // Ideally, I want to create a LocalSpan instead of Span here, and use it with in_span
    // This Span is not reported to the collector.
    call_nested_future_ext()
        .in_span(Span::enter_with_local_parent("in_span_async"))
        .await;

    {
        let _guard = LocalSpan::enter_with_local_parent("nested_wrapped");
        nested_wrapped().await
    }
}

async fn call_nested_future_ext() {}

async fn nested_wrapped() {}

#[event(start)]
fn start() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

#[event(fetch)]
async fn main(req: Request, env: Env, ctx: Context) -> Result<Response> {
    log("started");
    let collector = LocalCollector::start();

    {
        let _guard = LocalSpan::enter_with_local_parent("root");
        func_with_trace().await;
    }

    ctx.wait_until(async move {
        log("flushing in background");
        let local_spans = collector.collect();
        let span_context = SpanContext::new(TraceId(1), SpanId(1));
        let span_records = local_spans.to_span_records(span_context);
        log(format!("span_records: {:#?}", span_records).as_str());

        // The output is (only spans created with `#[trace]` or manually entered are collected):
        // TL;DR: root, child, nested_wrapped

        // SpanRecord {
        //     trace_id: TraceId(
        //         1,
        //     ),
        //     span_id: SpanId(
        //         13461468910679228417,
        //     ),
        //     parent_id: SpanId(
        //         1,
        //     ),
        //     begin_time_unix_ns: 1707031610622070313,
        //     duration_ns: 976562,
        //     name: "root",
        //     properties: [],
        //     events: [],
        // },
        // SpanRecord {
        //     trace_id: TraceId(
        //         1,
        //     ),
        //     span_id: SpanId(
        //         13461468910679228418,
        //     ),
        //     parent_id: SpanId(
        //         13461468910679228417,
        //     ),
        //     begin_time_unix_ns: 1707031610623046875,
        //     duration_ns: 1953125,
        //     name: "child",
        //     properties: [],
        //     events: [],
        // },
        // SpanRecord {
        //     trace_id: TraceId(
        //         1,
        //     ),
        //     span_id: SpanId(
        //         13461468910679228419,
        //     ),
        //     parent_id: SpanId(
        //         13461468910679228418,
        //     ),
        //     begin_time_unix_ns: 1707031610623046875,
        //     duration_ns: 1953125,
        //     name: "nested_wrapped",
        //     properties: [],
        //     events: [],
        // },
    });

    Response::ok("Hello, World!")
}
