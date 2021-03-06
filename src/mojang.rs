//! For interacting with the various Mojang APIs
//!
//! Remember that requests are rate limited, avoid sending too many requests,
//! and cache what you can.
//!
//! In general, you may want to read [wiki.vg/Mojang
//! API](http://wiki.vg/Mojang_API),
//! [wiki.vg/Authentication](http://wiki.vg/Authentication) and
//! [wiki.vg/Protocol
//! Encryption](http://wiki.vg/Protocol_Encryption#Authentication) for further
//! documentation about the
//! requests and their responses.
//!
//! Also contains some helper functions used for authentication.

pub use json::*;
use errors::Result;
use utils;

use curl::easy::{Easy, List};

use serde_json;

/// Make a request to check the status of the Mojang APIs
#[derive(Debug, Clone)]
pub struct APIStatus();
impl APIStatus {
    pub fn perform(&self) -> Result<APIStatusResponse> {
        let res = get_request(&Self::get_endpoint())?;
        /* Flatten the list, and turn it into an object.
         * For some reason this response is given in a really weird way, and
         * this fixes it so that it can be parsed more easily */
        let res = res.replace(|c| match c {
                                  '{' | '}' => true,
                                  _ => false,
                              },
                              "")
            .replace('[', "{")
            .replace(']', "}");
        Ok(serde_json::from_str(&res)?)
    }
    pub fn new() -> Self {
        APIStatus {}
    }
    fn get_endpoint() -> String {
        "https://status.mojang.com/check".to_string()
    }
}

/// Make a Username -> UUID (at time) request
///
/// Returns information about which account had the given name at the point in
/// time, where the time is specified as epoch time. If at is not specified, it
/// will default to the current time.
///
/// If unable to find the player at the given point in time, will return an
/// error.
#[derive(Debug, Clone)]
pub struct NameToUUID {
    username: String,
    at: Option<i64>,
}
impl NameToUUID {
    pub fn perform(&self) -> Result<NameUUID> {
        let url = match self.at {
            Some(x) => {
                format!("https://api.mojang.com/users/profiles/minecraft/{}?at={}",
                        self.username,
                        x)
            },
            None => {
                format!("https://api.mojang.com/users/profiles/minecraft/{}",
                        self.username)
            },
        };
        let res = get_request(&url)?;
        Ok(serde_json::from_str(&res)?)
    }
    pub fn new(username: String, at: Option<i64>) -> Self {
        NameToUUID {
            username: username,
            at: at,
        }
    }
}

/// A UUID -> Username history request
///
/// The UUID must be given as a string without hyphens.
#[derive(Debug, Clone)]
pub struct UUIDToHistory {
    uuid: String,
}
impl UUIDToHistory {
    pub fn perform(&self) -> Result<Vec<NameHistory>> {
        let url = format!("https://api.mojang.com/user/profiles/{}/names",
                          self.uuid);
        let res = get_request(&url)?;
        Ok(serde_json::from_str(&res)?)
    }
    pub fn new(uuid: String) -> Self {
        UUIDToHistory {
            uuid: uuid,
        }
    }
}

/// A Playernames -> UUIDs request.
///
/// Can request up to 100 UUIDs at a time.
#[derive(Debug, Clone)]
pub struct PlayernamesToUUIDs {
    usernames: Vec<String>,
}
impl PlayernamesToUUIDs {
    fn get_endpoint() -> String {
        "https://api.mojang.com/profiles/minecraft".to_string()
    }
    pub fn perform(&self) -> Result<Vec<NameUUID>> {
        let body = serde_json::to_string(&self.usernames)?;
        println!("body: {}", body);
        let res = post_request(&Self::get_endpoint(), &body)?;
        Ok(serde_json::from_str(&res)?)
    }
    /// Create a new instance of this request.
    ///
    /// # Panics
    ///
    /// Panics if usernames.len() > 100. The API limits this request to 100
    /// usernames.
    pub fn new(usernames: Vec<String>) -> Self {
        if usernames.len() > 100 {
            panic!("PlayernamesToUUIDs got more than 100 usernames");
        }
        PlayernamesToUUIDs {
            usernames: usernames,
        }
    }
}

/// Represents a UUID -> Profile + Skin and Cape request
#[derive(Debug, Clone)]
pub struct UUIDToProfile {
    uuid: String,
    /// Whether you want the response signed by the yggdrasil private key
    signed: bool,
}
impl UUIDToProfile {
    pub fn perform(&self) -> Result<Profile> {
        let url = if self.signed {
            format!("https://sessionserver.mojang.com/session/minecraft/profile/{}?unsigned=false",
                    self.uuid)
        } else {
            format!("https://sessionserver.mojang.com/session/minecraft/profile/{}",
                    self.uuid)
        };
        let res = get_request(&url)?;
        println!("res: {}", res);
        Ok(serde_json::from_str(&res)?)
    }
    pub fn new(uuid: String, signed: bool) -> Self {
        UUIDToProfile {
            uuid: uuid,
            signed: signed,
        }
    }
}

/// Get the blocked server's hashes
#[derive(Debug, Clone)]
pub struct BlockedServers();
impl BlockedServers {
    fn get_endpoint() -> String {
        "https://sessionserver.mojang.com/blockedservers".to_string()
    }
    pub fn perform(&self) -> Result<Vec<String>> {
        let res: String = get_request(&Self::get_endpoint())?;
        Ok(res.split('\n')
               .filter_map(|e| if !e.is_empty() {
                               Some(e.to_string())
                           } else {
                               None
                           })
               .collect())
    }
    pub fn new() -> Self {
        BlockedServers {}
    }
}

/// Get the orders statistics
///
/// The API will respond with the sum of sales for the selected types. E.g. by
/// setting item_sold_minecraft and prepaid_card_redeemed_minecraft to true,
/// you'll get the sum of sales for those two types.
#[derive(Debug, Clone)]
pub struct Statistics {
    item_sold_minecraft: bool,
    prepaid_card_redeemed_minecraft: bool,
    item_sold_cobalt: bool,
    item_sold_scrolls: bool,
}
impl Statistics {
    fn get_endpoint() -> String {
        "https://api.mojang.com/orders/statistics".to_string()
    }
    pub fn perform(&self) -> Result<StatisticsResponse> {
        let mut query: Vec<&str> = Vec::new();
        if self.item_sold_minecraft {
            query.push("item_sold_minecraft");
        }
        if self.prepaid_card_redeemed_minecraft {
            query.push("prepaid_card_redeemed_minecraft");
        }
        if self.item_sold_cobalt {
            query.push("item_sold_cobalt");
        }
        if self.item_sold_scrolls {
            query.push("item_sold_scrolls");
        }
        let payload = json!({
                                "metricKeys": query
                            });
        let res = post_request(&Self::get_endpoint(), &payload.to_string())?;
        Ok(serde_json::from_str(&res)?)
    }
    /// Create a new request for requesting the sum of sales of the specified
    /// types.
    ///
    /// # Panics
    ///
    /// Panics if not at least one of the values is true.
    pub fn new(item_sold_minecraft: bool,
               prepaid_card_redeemed_minecraft: bool,
               item_sold_cobalt: bool,
               item_sold_scrolls: bool)
               -> Self {
        if !(item_sold_minecraft | prepaid_card_redeemed_minecraft |
             item_sold_cobalt | item_sold_scrolls) {
            panic!("You must specify at least one type of sale in the Statistics request");
        }
        Statistics {
            item_sold_minecraft: item_sold_minecraft,
            prepaid_card_redeemed_minecraft: prepaid_card_redeemed_minecraft,
            item_sold_cobalt: item_sold_cobalt,
            item_sold_scrolls: item_sold_scrolls,
        }
    }
    /// Get the sum of everything
    pub fn all() -> Self {
        Statistics {
            item_sold_minecraft: true,
            prepaid_card_redeemed_minecraft: true,
            item_sold_cobalt: true,
            item_sold_scrolls: true,
        }
    }
    /// Get just the amount of Minecraft sales
    pub fn minecraft() -> Self {
        Statistics {
            item_sold_minecraft: true,
            prepaid_card_redeemed_minecraft: true,
            item_sold_cobalt: false,
            item_sold_scrolls: false,
        }
    }
}

/* Here begins the authentication requests */

/// Authenticate with Mojang
#[derive(Debug, Clone)]
pub struct Authenticate {
    username: String,
    password: String,
    clientToken: Option<String>,
    requestUser: bool,
}
impl Authenticate {
    fn get_endpoint() -> String {
        "https://authserver.mojang.com/authenticate".to_string()
    }
    pub fn perform(&self) -> Result<AuthenticationResponse> {
        let payload = json!({
            "agent": {
                "name": "Minecraft",
                "version": 1
            },
            "username": self.username,
            "password": self.password,
            "clientToken": self.clientToken,
            "requestUser": self.requestUser
        });
        let res = post_request(&Self::get_endpoint(), &payload.to_string())?;
        Ok(serde_json::from_str(&res)?)
    }
    pub fn new(username: String, password: String) -> Self {
        Authenticate {
            username: username,
            password: password,
            clientToken: None,
            requestUser: false,
        }
    }
}

/// Refresh a valid accessToken
#[derive(Debug, Serialize, Clone)]
pub struct AuthenticateRefresh {
    accessToken: String,
    clientToken: String,
    requestUser: bool,
}
impl AuthenticateRefresh {
    fn get_endpoint() -> String {
        "https://authserver.mojang.com/refresh".to_string()
    }
    pub fn perform(&self) -> Result<AuthenticationResponse> {
        let payload = serde_json::to_string(self)?;
        let res = post_request(&Self::get_endpoint(), &payload)?;
        Ok(serde_json::from_str(&res)?)
    }
    pub fn new(accessToken: String,
               clientToken: String,
               requestUser: bool)
               -> Self {
        AuthenticateRefresh {
            accessToken: accessToken,
            clientToken: clientToken,
            requestUser: requestUser,
        }
    }
}

/// Validate an existing access token
#[derive(Debug, Serialize, Clone)]
pub struct AuthenticateValidate {
    accessToken: String,
    clientToken: Option<String>,
}
impl AuthenticateValidate {
    fn get_endpoint() -> String {
        "https://authserver.mojang.com/validate".to_string()
    }
    pub fn perform(&self) -> Result<()> {
        let payload = serde_json::to_string(self)?;
        let _ = post_request(&Self::get_endpoint(), &payload)?;
        Ok(())
    }
    pub fn new(accessToken: String, clientToken: Option<String>) -> Self {
        AuthenticateValidate {
            accessToken: accessToken,
            clientToken: clientToken,
        }
    }
}

/// Invalidate an accessToken, using the client username/password
#[derive(Debug, Serialize, Clone)]
pub struct AuthenticateSignout {
    username: String,
    password: String,
}
impl AuthenticateSignout {
    fn get_endpoint() -> String {
        "https://authserver.mojang.com/signout".to_string()
    }
    pub fn perform(&self) -> Result<()> {
        let payload = serde_json::to_string(self)?;
        let _ = post_request(&Self::get_endpoint(), &payload)?;
        Ok(())
    }
    pub fn new(username: String, password: String) -> Self {
        AuthenticateSignout {
            username: username,
            password: password,
        }
    }
}

/// Invalidate an accessToken, using the accessToken and a clientToken
#[derive(Debug, Serialize, Clone)]
pub struct AuthenticateInvalidate {
    accessToken: String,
    clientToken: String,
}
impl AuthenticateInvalidate {
    fn get_endpoint() -> String {
        "https://authserver.mojang.com/invalidate".to_string()
    }
    pub fn perform(&self) -> Result<()> {
        let payload = serde_json::to_string(self)?;
        let _ = post_request(&Self::get_endpoint(), &payload)?;
        Ok(())
    }
    pub fn new(accessToken: String, clientToken: String) -> Self {
        AuthenticateInvalidate {
            accessToken: accessToken,
            clientToken: clientToken,
        }
    }
}

/// Send a session join message to Mojang, used by clients when connecting to
/// online servers
#[derive(Debug, Serialize, Clone)]
pub struct SessionJoin {
    accessToken: String,
    /// The player's uuid
    selectedProfile: String,
    serverId: String,
}
impl SessionJoin {
    fn get_endpoint() -> String {
        "https://sessionserver.mojang.com/session/minecraft/join".to_string()
    }
    pub fn perform(&self) -> Result<()> {
        let payload = serde_json::to_string(self)?;
        let _ = post_request(&Self::get_endpoint(), &payload)?;
        Ok(())
    }
    pub fn new(access_token: String,
               uuid: String,
               server_id: &str,
               shared_secret: &[u8],
               server_public_key: &[u8])
               -> Self {
        let hash =
            utils::post_sha1(server_id, shared_secret, server_public_key);
        SessionJoin {
            accessToken: access_token,
            selectedProfile: uuid,
            serverId: hash,
        }
    }
}

/// Check whether a client has posted a SessionJoin to Mojang, used by servers
/// for authenticating connecting clients.
#[derive(Debug, Clone)]
pub struct SessionHasJoined {
    username: String,
    serverId: String,
}
impl SessionHasJoined {
    pub fn perform(&self) -> Result<SessionHasJoinedResponse> {
        let url = format!("https://sessionserver.mojang.com/session/minecraft/hasJoined?username={}&serverId={}", self.username, self.serverId);
        let res = get_request(&url)?;
        println!("session has joined response: {}", &res);
        Ok(serde_json::from_str(&res)?)
    }
    pub fn new(username: String,
               server_id: &str,
               shared_secret: &[u8],
               public_key: &[u8])
               -> Self {
        let hash = utils::post_sha1(server_id, shared_secret, public_key);
        SessionHasJoined {
            username: username,
            serverId: hash,
        }
    }
}

/// Helper function for performing a GET request to the given URL, returning
/// the response content
fn get_request(url: &str) -> Result<String> {
    let mut handle = Easy::new();
    handle.url(url)?;
    handle.fail_on_error(true)?;
    let mut response = Vec::new();
    {
        let mut transfer = handle.transfer();
        transfer
            .write_function(|data| {
                                response.extend_from_slice(data);
                                Ok(data.len())
                            })?;
        transfer.perform()?;
    }
    Ok(String::from_utf8(response)?)
}

/// Helper function for performing a POST request to the given URL,
/// posting the given data to it, and returning the response content.
fn post_request(url: &str, post: &str) -> Result<String> {
    let mut handle = Easy::new();
    handle.url(url)?;
    handle.fail_on_error(true)?;
    let mut headers = List::new();
    headers.append("Content-Type: application/json")?;
    handle.http_headers(headers)?;
    handle.post_fields_copy(post.as_bytes())?;
    handle.post(true)?;
    let mut response = Vec::new();
    {
        let mut transfer = handle.transfer();
        transfer
            .write_function(|data| {
                                response.extend_from_slice(data);
                                Ok(data.len())
                            })?;
        transfer.perform()?;
    }
    Ok(String::from_utf8(response)?)
}
