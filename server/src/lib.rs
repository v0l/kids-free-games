mod db;
mod error;
mod games;
mod handlers;
mod routes;
mod rules;

use db::Db;
use error::{ApiError, ApiResult};
use routes::Endpoint;
use serde::de::DeserializeOwned;
use serde_json::Value;
use worker::{event, Context, Date, Env, Request, Response, Result};

fn now_secs() -> i64 {
    (Date::now().as_millis() / 1000) as i64
}

fn cors(mut res: Response) -> Result<Response> {
    let h = res.headers_mut();
    h.set("Access-Control-Allow-Origin", "*")?;
    h.set(
        "Access-Control-Allow-Methods",
        "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    )?;
    h.set(
        "Access-Control-Allow-Headers",
        "Authorization, Content-Type",
    )?;
    h.set("Access-Control-Max-Age", "86400")?;
    h.set("Cache-Control", "no-store")?;
    Ok(res)
}

fn bearer(req: &Request) -> Option<String> {
    let v = req.headers().get("Authorization").ok().flatten()?;
    let (scheme, token) = v.split_once(' ')?;
    scheme
        .eq_ignore_ascii_case("bearer")
        .then(|| token.trim().to_string())
}

async fn body<T: DeserializeOwned>(req: &mut Request) -> ApiResult<T> {
    let declared = req
        .headers()
        .get("Content-Length")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<usize>().ok());
    if declared.is_some_and(|n| n > rules::MAX_BODY_BYTES) {
        return Err(ApiError::too_large("request body is too large"));
    }
    let text = req.text().await?;
    if text.len() > rules::MAX_BODY_BYTES {
        return Err(ApiError::too_large("request body is too large"));
    }
    Ok(serde_json::from_str(&text)?)
}

async fn limited(env: &Env, binding: &str, key: String) -> ApiResult<()> {
    let Ok(limiter) = env.rate_limiter(binding) else {
        return Ok(());
    };
    let outcome = limiter.limit(key).await?;
    if outcome.success {
        Ok(())
    } else {
        Err(ApiError::rate_limited())
    }
}

fn client_ip(req: &Request) -> String {
    req.headers()
        .get("CF-Connecting-IP")
        .ok()
        .flatten()
        .unwrap_or_else(|| "local".into())
}

fn query(req: &Request, key: &str) -> Option<String> {
    req.url()
        .ok()?
        .query_pairs()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

async fn route(mut req: Request, env: &Env) -> ApiResult<Value> {
    let endpoint = routes::resolve(&req.method(), &req.path())?;
    let now = now_secs();
    let db = Db(env.d1("DB")?);

    match &endpoint {
        Endpoint::Health => return Ok(serde_json::json!({ "ok": true, "time": now })),
        Endpoint::Games => return Ok(handlers::games_list()),
        Endpoint::Register => {
            limited(env, "RL_REGISTER", client_ip(&req)).await?;
            let b = body(&mut req).await?;
            return handlers::register(&db, b, now).await;
        }
        Endpoint::RedeemLink => {
            limited(env, "RL_LINK", client_ip(&req)).await?;
            let b = body(&mut req).await?;
            return handlers::redeem_link(&db, b, now).await;
        }
        _ => {}
    }

    let auth = handlers::authenticate(&db, bearer(&req).as_deref(), now).await?;
    limited(env, "RL_API", auth.player_id.clone()).await?;

    match endpoint {
        Endpoint::GetMe => handlers::get_me(&db, &auth).await,
        Endpoint::PatchMe => {
            let b = body(&mut req).await?;
            handlers::patch_me(&db, &auth, b).await
        }
        Endpoint::DeleteMe => handlers::delete_me(&db, &auth).await,
        Endpoint::CreateLink => handlers::create_link(&db, &auth, now).await,
        Endpoint::ListSaves => handlers::list_saves(&db, &auth).await,
        Endpoint::GetSave(slot) => handlers::get_save(&db, &auth, &slot).await,
        Endpoint::PutSave(slot) => {
            let b = body(&mut req).await?;
            handlers::put_save(&db, &auth, &slot, b, now).await
        }
        Endpoint::GetBoard(game) => {
            handlers::get_board(&db, &auth, &game, query(&req, "limit").as_deref()).await
        }
        Endpoint::PostScore(game) => {
            let b = body(&mut req).await?;
            handlers::post_score(&db, &auth, &game, b, now).await
        }
        Endpoint::GetPlaytime => {
            handlers::get_playtime(&db, &auth, query(&req, "days").as_deref(), now).await
        }
        Endpoint::PostPlaytime => {
            let b = body(&mut req).await?;
            handlers::post_playtime(&db, &auth, b, now).await
        }
        Endpoint::Health | Endpoint::Games | Endpoint::Register | Endpoint::RedeemLink => {
            Err(ApiError::internal("public endpoint reached auth"))
        }
    }
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    if req.method() == worker::Method::Options {
        return cors(Response::empty()?.with_status(204));
    }
    let res = match route(req, &env).await {
        Ok(v) => Response::from_json(&v)?,
        Err(e) => e.into_response()?,
    };
    cors(res)
}
