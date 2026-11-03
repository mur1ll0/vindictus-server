//! Sessão do launcher no servidor (listener do painel, `src/http/admin.rs`):
//! `POST /api/launcher/login` (login e senha → sessão), `POST /api/launcher/ticket` (Bearer → ticket
//! de uso único, válido enquanto a sessão durar) e `POST /api/launcher/logout` (Bearer). A senha, a
//! sessão e o ticket ficam em `Zeroizing` e nunca vão para log nem disco.
use serde::Deserialize;
use std::time::Duration;
use zeroize::Zeroizing;

#[derive(Debug, PartialEq, Eq)]
pub enum ApiError {
    /// Login ou senha errados (401 no login).
    InvalidCredentials,
    /// A sessão acabou: logout, senha trocada, conta desativada ou limite de 12 horas.
    SessionEnded,
    RateLimited,
    /// O servidor não respondeu (desligado, endereço errado, rede).
    Offline,
    Server(u16),
    BadReply,
}

impl ApiError {
    pub fn message(&self) -> String {
        match self {
            ApiError::InvalidCredentials => "Login ou senha inválidos.".into(),
            ApiError::SessionEnded => "Sua sessão terminou. Entre de novo.".into(),
            ApiError::RateLimited => "Muitas tentativas. Aguarde um minuto.".into(),
            ApiError::Offline => "Servidor fora do ar ou endereço errado.".into(),
            ApiError::Server(code) => format!("O servidor recusou o pedido (HTTP {code})."),
            ApiError::BadReply => "Resposta inesperada do servidor.".into(),
        }
    }
}

/// Sessão do launcher (token Bearer). Não é Debug: é uma credencial.
pub struct Session(Zeroizing<String>);

/// Ticket de jogo de uso único. Não é Debug: é uma credencial.
pub struct Ticket(Zeroizing<String>);

impl Ticket {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Deserialize)]
struct SessionReply {
    session: Zeroizing<String>,
}

#[derive(Deserialize)]
struct TicketReply {
    ticket: Zeroizing<String>,
}

#[derive(Clone)]
pub struct Api {
    base: String,
    agent: ureq::Agent,
}

impl Api {
    pub fn new(base: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .user_agent(concat!("vindictus-launcher/", env!("CARGO_PKG_VERSION")))
            .build();
        Self {
            base: base.trim_end_matches('/').to_owned(),
            agent,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    pub fn panel_url(&self) -> String {
        self.url("/")
    }

    pub fn register_url(&self) -> String {
        self.url("/login?cadastro=1")
    }

    /// O listener responde (qualquer código HTTP conta como no ar).
    pub fn online(&self) -> bool {
        match self.agent.get(&self.url("/api/session")).call() {
            Ok(_) | Err(ureq::Error::Status(..)) => true,
            Err(ureq::Error::Transport(_)) => false,
        }
    }

    pub fn login(&self, username: &str, password: &Zeroizing<String>) -> Result<Session, ApiError> {
        let body = Zeroizing::new(
            serde_json::json!({ "username": username, "password": password.as_str() }).to_string(),
        );
        let reply = self
            .agent
            .post(&self.url("/api/launcher/login"))
            .set("Content-Type", "application/json")
            .send_string(&body)
            .map_err(|e| map_error(e, ApiError::InvalidCredentials))?;
        let reply: SessionReply = reply.into_json().map_err(|_| ApiError::BadReply)?;
        Ok(Session(reply.session))
    }

    pub fn ticket(&self, session: &Session) -> Result<Ticket, ApiError> {
        let reply = self
            .agent
            .post(&self.url("/api/launcher/ticket"))
            .set("Authorization", &bearer(session))
            .call()
            .map_err(|e| map_error(e, ApiError::SessionEnded))?;
        let reply: TicketReply = reply.into_json().map_err(|_| ApiError::BadReply)?;
        Ok(Ticket(reply.ticket))
    }

    /// Encerra a sessão e os tickets não usados. Uma sessão já encerrada não é erro.
    pub fn logout(&self, session: &Session) -> Result<(), ApiError> {
        match self
            .agent
            .post(&self.url("/api/launcher/logout"))
            .set("Authorization", &bearer(session))
            .call()
        {
            Ok(_) | Err(ureq::Error::Status(401, _)) => Ok(()),
            Err(e) => Err(map_error(e, ApiError::SessionEnded)),
        }
    }
}

fn bearer(session: &Session) -> Zeroizing<String> {
    Zeroizing::new(format!("Bearer {}", session.0.as_str()))
}

fn map_error(error: ureq::Error, unauthorized: ApiError) -> ApiError {
    match error {
        ureq::Error::Status(401, _) => unauthorized,
        ureq::Error::Status(429, _) => ApiError::RateLimited,
        ureq::Error::Status(code, _) => ApiError::Server(code),
        ureq::Error::Transport(_) => ApiError::Offline,
    }
}
