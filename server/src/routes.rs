use crate::error::{ApiError, ApiResult};
use worker::Method;

#[derive(Debug, PartialEq, Eq)]
pub enum Endpoint {
    Health,
    Games,
    Register,
    RedeemLink,
    GetMe,
    PatchMe,
    DeleteMe,
    CreateLink,
    ListSaves,
    GetSave(String),
    PutSave(String),
    GetBoard(String),
    PostScore(String),
    GetPlaytime,
    PostPlaytime,
}

pub fn resolve(method: &Method, path: &str) -> ApiResult<Endpoint> {
    let trimmed = path.trim_end_matches('/');
    let segments: Vec<&str> = trimmed.split('/').skip(1).collect();
    let get = *method == Method::Get || *method == Method::Head;
    let post = *method == Method::Post;
    let put = *method == Method::Put;
    let patch = *method == Method::Patch;
    let delete = *method == Method::Delete;
    let pick = |allowed: bool, e: Endpoint| {
        if allowed {
            Ok(e)
        } else {
            Err(ApiError::method_not_allowed())
        }
    };
    match segments.as_slice() {
        ["v1", "health"] => pick(get, Endpoint::Health),
        ["v1", "games"] => pick(get, Endpoint::Games),
        ["v1", "players"] => pick(post, Endpoint::Register),
        ["v1", "me"] => match () {
            _ if get => Ok(Endpoint::GetMe),
            _ if patch => Ok(Endpoint::PatchMe),
            _ if delete => Ok(Endpoint::DeleteMe),
            _ => Err(ApiError::method_not_allowed()),
        },
        ["v1", "link"] => pick(post, Endpoint::CreateLink),
        ["v1", "link", "redeem"] => pick(post, Endpoint::RedeemLink),
        ["v1", "saves"] => pick(get, Endpoint::ListSaves),
        ["v1", "saves", slot] => match () {
            _ if get => Ok(Endpoint::GetSave(slot.to_string())),
            _ if put => Ok(Endpoint::PutSave(slot.to_string())),
            _ => Err(ApiError::method_not_allowed()),
        },
        ["v1", "scores", game] => match () {
            _ if get => Ok(Endpoint::GetBoard(game.to_string())),
            _ if post => Ok(Endpoint::PostScore(game.to_string())),
            _ => Err(ApiError::method_not_allowed()),
        },
        ["v1", "playtime"] => match () {
            _ if get => Ok(Endpoint::GetPlaytime),
            _ if post => Ok(Endpoint::PostPlaytime),
            _ => Err(ApiError::method_not_allowed()),
        },
        _ => Err(ApiError::not_found("no such endpoint")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routes_resolve_by_method_and_path() {
        assert_eq!(
            resolve(&Method::Get, "/v1/health").unwrap(),
            Endpoint::Health
        );
        assert_eq!(
            resolve(&Method::Post, "/v1/players").unwrap(),
            Endpoint::Register
        );
        assert_eq!(
            resolve(&Method::Patch, "/v1/me").unwrap(),
            Endpoint::PatchMe
        );
        assert_eq!(
            resolve(&Method::Put, "/v1/saves/profile").unwrap(),
            Endpoint::PutSave("profile".into())
        );
        assert_eq!(
            resolve(&Method::Post, "/v1/scores/snake/").unwrap(),
            Endpoint::PostScore("snake".into())
        );
        assert_eq!(
            resolve(&Method::Post, "/v1/link/redeem").unwrap(),
            Endpoint::RedeemLink
        );
        assert_eq!(
            resolve(&Method::Post, "/v1/link").unwrap(),
            Endpoint::CreateLink
        );
    }

    #[test]
    fn wrong_method_is_405_and_unknown_path_is_404() {
        assert_eq!(
            resolve(&Method::Delete, "/v1/saves/x").unwrap_err().status,
            405
        );
        assert_eq!(
            resolve(&Method::Get, "/v1/players").unwrap_err().status,
            405
        );
        assert_eq!(resolve(&Method::Get, "/v1/nope").unwrap_err().status, 404);
        assert_eq!(resolve(&Method::Get, "/").unwrap_err().status, 404);
        assert_eq!(resolve(&Method::Get, "/v2/health").unwrap_err().status, 404);
        assert_eq!(
            resolve(&Method::Get, "/v1/saves/a/b").unwrap_err().status,
            404
        );
    }
}
