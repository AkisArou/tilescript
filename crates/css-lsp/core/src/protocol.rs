use lsp_server::{ErrorCode, Message, Notification, Request, RequestId, Response};
use lsp_types::{
    CodeActionOptions, CodeActionParams, CodeActionProviderCapability, CompletionOptions,
    CompletionParams, DidChangeTextDocumentParams, DidCloseTextDocumentParams,
    DidOpenTextDocumentParams, DocumentSymbolParams, DocumentSymbolResponse, GotoDefinitionParams,
    HoverParams, HoverProviderCapability, InitializeParams, InitializeResult, MessageType, OneOf,
    ReferenceParams, RenameOptions, RenameParams, ServerCapabilities, ShowMessageParams,
    SymbolInformation, TextDocumentSyncCapability, TextDocumentSyncKind, WorkspaceSymbolParams,
    notification::Notification as NotificationTrait,
    notification::{DidChangeTextDocument, DidCloseTextDocument, DidOpenTextDocument, Initialized},
    request::Request as RequestTrait,
    request::{
        CodeActionRequest, Completion, DocumentSymbolRequest, GotoDefinition, HoverRequest,
        References, Rename, Shutdown, WorkspaceSymbolRequest,
    },
};
use serde::Serialize;

use crate::{Session, SessionChangeResult};

#[derive(Debug, Clone, serde::Serialize)]
pub struct ServerEvent {
    pub message: Message,
}

pub fn server_capabilities() -> ServerCapabilities {
    ServerCapabilities {
        text_document_sync: Some(TextDocumentSyncCapability::Kind(TextDocumentSyncKind::FULL)),
        completion_provider: Some(CompletionOptions::default()),
        code_action_provider: Some(CodeActionProviderCapability::Options(CodeActionOptions {
            ..CodeActionOptions::default()
        })),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        document_symbol_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        rename_provider: Some(OneOf::Right(RenameOptions {
            prepare_provider: Some(false),
            work_done_progress_options: Default::default(),
        })),
        workspace_symbol_provider: Some(OneOf::Left(true)),
        ..ServerCapabilities::default()
    }
}

pub fn handle_initialize(request: Request) -> (Response, Vec<ServerEvent>) {
    let session = Session::new();
    let response = parse_request(&session, request, |_, _: InitializeParams| {
        Ok(InitializeResult { capabilities: server_capabilities(), ..InitializeResult::default() })
    });
    (response, Vec::new())
}

pub fn handle_request(session: &Session, request: Request) -> (Response, Vec<ServerEvent>) {
    let method = request.method.clone();
    let response = match method.as_str() {
        Shutdown::METHOD => parse_request(session, request, |_, _: ()| Ok(())),
        HoverRequest::METHOD => parse_request(session, request, |session, params: HoverParams| {
            Ok(session.hover(
                &params.text_document_position_params.text_document.uri,
                params.text_document_position_params.position,
            ))
        }),
        Completion::METHOD => {
            parse_request(session, request, |session, params: CompletionParams| {
                Ok(session.completion(
                    &params.text_document_position.text_document.uri,
                    params.text_document_position.position,
                ))
            })
        }
        DocumentSymbolRequest::METHOD => {
            parse_request(session, request, |session, params: DocumentSymbolParams| {
                Ok(Some(DocumentSymbolResponse::Nested(
                    session.document_symbols(&params.text_document.uri),
                )))
            })
        }
        GotoDefinition::METHOD => {
            parse_request(session, request, |session, params: GotoDefinitionParams| {
                Ok(session.definition(
                    &params.text_document_position_params.text_document.uri,
                    params.text_document_position_params.position,
                ))
            })
        }
        References::METHOD => {
            parse_request(session, request, |session, params: ReferenceParams| {
                Ok(Some(session.references(
                    &params.text_document_position.text_document.uri,
                    params.text_document_position.position,
                    params.context.include_declaration,
                )))
            })
        }
        Rename::METHOD => parse_request(session, request, |session, params: RenameParams| {
            Ok(session.rename(
                &params.text_document_position.text_document.uri,
                params.text_document_position.position,
                &params.new_name,
            ))
        }),
        CodeActionRequest::METHOD => {
            parse_request(session, request, |session, params: CodeActionParams| {
                Ok(Some(
                    session.code_actions(&params.text_document.uri, &params.context.diagnostics),
                ))
            })
        }
        WorkspaceSymbolRequest::METHOD => {
            parse_request(session, request, |session, params: WorkspaceSymbolParams| {
                Ok(Some::<Vec<SymbolInformation>>(session.workspace_symbols(&params.query)))
            })
        }
        _ => Response::new_err(
            request.id,
            ErrorCode::MethodNotFound as i32,
            format!("unsupported request: {}", method),
        ),
    };

    (response, Vec::new())
}

pub fn handle_notification(session: &mut Session, notification: Notification) -> Vec<ServerEvent> {
    match notification.method.as_str() {
        Initialized::METHOD => vec![log_message("tilescript-css-lsp initialized")],
        DidOpenTextDocument::METHOD => {
            parse_notification::<DidOpenTextDocumentParams>(notification, |params| {
                let document = params.text_document;
                diagnostics_events(session.open(document.uri, document.text))
            })
        }
        DidChangeTextDocument::METHOD => {
            parse_notification::<DidChangeTextDocumentParams>(notification, |params| {
                let uri = params.text_document.uri;
                let Some(change) = params.content_changes.into_iter().last() else {
                    return Vec::new();
                };
                diagnostics_events(session.change(uri, change.text))
            })
        }
        DidCloseTextDocument::METHOD => {
            parse_notification::<DidCloseTextDocumentParams>(notification, |params| {
                diagnostics_events(session.close(&params.text_document.uri))
            })
        }
        _ => Vec::new(),
    }
}

fn diagnostics_events(result: SessionChangeResult) -> Vec<ServerEvent> {
    vec![ServerEvent {
        message: Message::Notification(Notification::new(
            "textDocument/publishDiagnostics".to_string(),
            lsp_types::PublishDiagnosticsParams {
                uri: result.uri,
                diagnostics: result.diagnostics,
                version: None,
            },
        )),
    }]
}

fn log_message(message: impl Into<String>) -> ServerEvent {
    ServerEvent {
        message: Message::Notification(Notification::new(
            "window/logMessage".to_string(),
            ShowMessageParams { typ: MessageType::INFO, message: message.into() },
        )),
    }
}

fn parse_request<P, R>(
    session: &Session,
    request: Request,
    handler: impl FnOnce(&Session, P) -> Result<R, String>,
) -> Response
where
    P: serde::de::DeserializeOwned,
    R: Serialize,
{
    let Request { id, params, .. } = request;
    let params = match serde_json::from_value::<P>(params) {
        Ok(params) => params,
        Err(error) => return invalid_params_response(id, error.to_string()),
    };
    match handler(session, params) {
        Ok(result) => Response::new_ok(id, result),
        Err(error) => Response::new_err(id, ErrorCode::InternalError as i32, error),
    }
}

fn parse_notification<P>(
    notification: Notification,
    handler: impl FnOnce(P) -> Vec<ServerEvent>,
) -> Vec<ServerEvent>
where
    P: serde::de::DeserializeOwned,
{
    serde_json::from_value(notification.params).map(handler).unwrap_or_default()
}

fn invalid_params_response(id: RequestId, message: String) -> Response {
    Response::new_err(id, ErrorCode::InvalidParams as i32, message)
}
