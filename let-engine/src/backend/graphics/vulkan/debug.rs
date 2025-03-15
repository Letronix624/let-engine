use anyhow::{Context, Result};
use std::sync::Arc;
use vulkano::instance::{debug::*, Instance};

use log::{error, info, warn};

pub fn make_debug(instance: &Arc<Instance>) -> Result<DebugUtilsMessenger> {
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
                        let ty = if message_type.intersects(DebugUtilsMessageType::GENERAL) {
                            "general"
                        } else if message_type.intersects(DebugUtilsMessageType::VALIDATION) {
                            "validation"
                        } else if message_type.intersects(DebugUtilsMessageType::PERFORMANCE) {
                            "performance"
                        } else {
                            panic!("no-impl");
                        };

                        if severity.intersects(DebugUtilsMessageSeverity::ERROR) {
                            error!(
                                "{} {}: {}",
                                callback_data.message_id_name.unwrap_or("unknown"),
                                ty,
                                callback_data.message
                            );
                        } else if severity.intersects(DebugUtilsMessageSeverity::WARNING) {
                            warn!(
                                "{} {}: {}",
                                callback_data.message_id_name.unwrap_or("unknown"),
                                ty,
                                callback_data.message
                            );
                        } else if severity.intersects(DebugUtilsMessageSeverity::INFO) {
                            info!(
                                "{} {}: {}",
                                callback_data.message_id_name.unwrap_or("unknown"),
                                ty,
                                callback_data.message
                            );
                        } else if severity.intersects(DebugUtilsMessageSeverity::VERBOSE) {
                            info!(
                                "{} {} verbose: {}",
                                callback_data.message_id_name.unwrap_or("unknown"),
                                ty,
                                callback_data.message
                            );
                        };

                    },
                ))
            },
        ).context("There was a problem setting up a vulkan debug reporter. Consider turning off the `vulkan_debug` feature for this build.")
    }
}
