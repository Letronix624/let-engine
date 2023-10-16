use vulkano::instance::{debug::*, Instance};
use std::sync::Arc;

pub fn make_debug(instance: &Arc<Instance>) -> DebugUtilsMessenger {
    unsafe {
        DebugUtilsMessenger::new(
            instance.clone(),
            DebugUtilsMessengerCreateInfo {
                message_severity: DebugUtilsMessageSeverity::ERROR
                    | DebugUtilsMessageSeverity::WARNING
                    | DebugUtilsMessageSeverity::INFO
                    | DebugUtilsMessageSeverity::VERBOSE,
                message_type: DebugUtilsMessageType::GENERAL
                    | DebugUtilsMessageType::VALIDATION
                    | DebugUtilsMessageType::PERFORMANCE,
                ..DebugUtilsMessengerCreateInfo::user_callback(Arc::new(|msg| {
                    let severity = if msg.severity.intersects(DebugUtilsMessageSeverity::ERROR) {
                        "error"
                    } else if msg.severity.intersects(DebugUtilsMessageSeverity::WARNING) {
                        "warning"
                    } else if msg.severity.intersects(DebugUtilsMessageSeverity::INFO) {
                        "information"
                    } else if msg.severity.intersects(DebugUtilsMessageSeverity::VERBOSE) {
                        "verbose"
                    } else {
                        panic!("no-impl");
                    };

                    let ty = if msg.ty.intersects(DebugUtilsMessageType::GENERAL) {
                        "general"
                    } else if msg.ty.intersects(DebugUtilsMessageType::VALIDATION) {
                        "validation"
                    } else if msg.ty.intersects(DebugUtilsMessageType::PERFORMANCE) {
                        "performance"
                    } else {
                        panic!("no-impl");
                    };

                    println!(
                        "{} {} {}: {}",
                        msg.layer_prefix.unwrap_or("unknown"),
                        ty,
                        severity,
                        msg.description
                    );
                }))
            },
        )
        .ok()
    }.unwrap()
}