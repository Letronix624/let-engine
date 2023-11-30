use std::sync::Arc;
use vulkano::instance::{debug::*, Instance};

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
                ..DebugUtilsMessengerCreateInfo::user_callback(DebugUtilsMessengerCallback::new(
                    |severity, message_type, callback_data| {
                        let severity = if severity.intersects(DebugUtilsMessageSeverity::ERROR) {
                            "error"
                        } else if severity.intersects(DebugUtilsMessageSeverity::WARNING) {
                            "warning"
                        } else if severity.intersects(DebugUtilsMessageSeverity::INFO) {
                            "information"
                        } else if severity.intersects(DebugUtilsMessageSeverity::VERBOSE) {
                            "verbose"
                        } else {
                            panic!("no-impl");
                        };

                        let ty = if message_type.intersects(DebugUtilsMessageType::GENERAL) {
                            "general"
                        } else if message_type.intersects(DebugUtilsMessageType::VALIDATION) {
                            "validation"
                        } else if message_type.intersects(DebugUtilsMessageType::PERFORMANCE) {
                            "performance"
                        } else {
                            panic!("no-impl");
                        };

                        println!(
                            "{} {} {}: {}",
                            callback_data.message_id_name.unwrap_or("unknown"),
                            ty,
                            severity,
                            callback_data.message
                        );
                    },
                ))
            },
        )
        .ok()
    }
    .unwrap()
}
