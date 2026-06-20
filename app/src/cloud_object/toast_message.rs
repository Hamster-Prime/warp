use warpui::AppContext;

use super::{CloudObject, GenericStringObjectFormat, JsonObjectType, ObjectType};
use crate::server::cloud_objects::update_manager::{
    InitiatedBy, ObjectOperation, OperationSuccessType,
};

pub struct CloudObjectToastMessage;

impl CloudObjectToastMessage {
    pub fn toast_message(
        object: &dyn CloudObject,
        operation: &ObjectOperation,
        success_type: &OperationSuccessType,
        app: &AppContext,
    ) -> Option<String> {
        let object_name = object.model_type_name().to_owned();
        let object_name_lowercase = object_name.to_ascii_lowercase();

        match (object.object_type(), operation, success_type) {
            // We should only show toasts for creates initiated by the user, not by the system
            (_, ObjectOperation::Create { initiated_by: InitiatedBy::User }, OperationSuccessType::Success) => {
                let containing_object_name = object.containing_object_name(app);
                Some(i18n::t!("{object_name} saved to {containing_object_name}", object_name = object_name, containing_object_name = containing_object_name).to_string())
            }
            // notebooks intentionally do not have an update message, as they are updated
            // as the user types and so toasts would be VERY noisy
            (
                ObjectType::Notebook,
                ObjectOperation::Update,
                OperationSuccessType::Success,
            ) => None,
            (_, ObjectOperation::Update, OperationSuccessType::Success) => {
                Some(i18n::t!("{object_name} updated", object_name = object_name).to_string())
            }
            (_, ObjectOperation::MoveToFolder, OperationSuccessType::Success) | (_, ObjectOperation::MoveToDrive, OperationSuccessType::Success) => {
                let containing_object_name = object.containing_object_name(app);
                Some(i18n::t!("{object_name} moved to {containing_object_name}", object_name = object_name, containing_object_name = containing_object_name).to_string())
            }
            (_, ObjectOperation::Trash, OperationSuccessType::Success) => {
                Some(i18n::t!("{object_name} trashed", object_name = object_name).to_string())
            }
            (_, ObjectOperation::Untrash, OperationSuccessType::Success) => {
                Some(i18n::t!("{object_name} restored", object_name = object_name).to_string())
            }
            (_, ObjectOperation::Leave, OperationSuccessType::Success) => {
                Some(i18n::t!("Left {object_name}", object_name = object_name).to_string())
            }
            (_, ObjectOperation::Create { initiated_by: InitiatedBy::User }, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to create {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            (_, ObjectOperation::Create { initiated_by: InitiatedBy::User }, OperationSuccessType::Denied(message)) => {
                Some(message.to_string())
            }
            (_, ObjectOperation::Update, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to update {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            (_, ObjectOperation::MoveToFolder, OperationSuccessType::Failure) | (_, ObjectOperation::MoveToDrive, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to move {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            (_, ObjectOperation::Trash, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to trash {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            (_, ObjectOperation::Untrash, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to restore {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            // We should only show deletion failure toasts for user-initiated deletions.
            (_, ObjectOperation::Delete { initiated_by: InitiatedBy::User }, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to delete {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            (_, ObjectOperation::Leave, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to leave {object_name}", object_name = object_name).to_string())
            }
            (
                ObjectType::Workflow,
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => {
                Some(i18n::t!("This workflow could not be saved because changes were made while you were editing.").to_string())
            }
            (
                ObjectType::GenericStringObject(GenericStringObjectFormat::Json(JsonObjectType::EnvVarCollection)),
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => {
                Some(i18n::t!("Environment variables could not be saved because changes were made while you were editing.").to_string())
            }
            (
                ObjectType::GenericStringObject(GenericStringObjectFormat::Json(JsonObjectType::AIFact)),
                ObjectOperation::Update,
                OperationSuccessType::Rejection,
            ) => {
                Some(i18n::t!("Rule could not be saved because changes were made while you were editing.").to_string())
            }
            (_, ObjectOperation::TakeEditAccess, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to start editing {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            (_, ObjectOperation::UpdatePermissions, OperationSuccessType::Success) => {
                Some(i18n::t!("Successfully updated permissions for {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            (_, ObjectOperation::UpdatePermissions, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to update permissions for {object_name_lowercase}", object_name_lowercase = object_name_lowercase).to_string())
            }
            _ => None,
        }
    }

    pub fn toast_deletion_confirm_message(
        num_objects: i32,
        operation: &ObjectOperation,
        success_type: &OperationSuccessType,
    ) -> Option<String> {
        let count_objects_message = match num_objects {
            1 => "1 object".to_string(),
            n => {
                i18n::t!("{n} objects", n = n).to_string()
            }
        };
        match (operation, success_type) {
            // We should only show deletion failure toasts for user-initiated deletions.
            (
                ObjectOperation::Delete {
                    initiated_by: InitiatedBy::User,
                },
                OperationSuccessType::Success,
            ) => Some(i18n::t!("{count_objects_message} deleted forever", count_objects_message = count_objects_message).to_string()),
            (ObjectOperation::EmptyTrash, OperationSuccessType::Success) => Some(format!(
                "Trash emptied: {count_objects_message} deleted forever"
            )),
            (ObjectOperation::EmptyTrash, OperationSuccessType::Failure) => {
                Some(i18n::t!("Failed to empty trash").to_string())
            }
            (ObjectOperation::EmptyTrash, OperationSuccessType::Rejection) => {
                Some(i18n::t!("No objects in trash to empty").to_string())
            }
            _ => None,
        }
    }
}
