use std::collections::HashMap;
use std::io::{Read, Result, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::Arc;

use bufstream::BufStream;
use openssl::ssl::{SslAcceptor, SslAcceptorBuilder, SslMethod, SslStream};

use error::ImapResult;
use self::config::Config;
use user::{load_users, Email, User};

mod config;
#[macro_use]
pub mod lmtp;

pub enum Stream {
    Ssl(SslStream<TcpStream>),
    Tcp(TcpStream)
}

impl Write for Stream {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match *self {
            Stream::Ssl(ref mut s) => s.write(buf),
            Stream::Tcp(ref mut s) => s.write(buf)
        }
    }

    fn flush(&mut self) -> Result<()> {
        match *self {
            Stream::Ssl(ref mut s) => s.flush(),
            Stream::Tcp(ref mut s) => s.flush()
        }
    }
}

impl Read for Stream {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match *self {
            Stream::Ssl(ref mut s) => s.read(buf),
            Stream::Tcp(ref mut s) => s.read(buf)
        }
    }
}

/// Holds configuration state and email->user map
pub struct Server {
    conf: Config,
    users: HashMap<Email, User>,
    ssl_acceptor: Option<SslAcceptor>,
}

impl Server {
    pub fn new() -> ImapResult<Server> {
        Server::new_with_conf(Config::new()?)
    }

    /// Create server to hold the Config and User HashMap
    fn new_with_conf(conf: Config) -> ImapResult<Server> {
        // Load the user data from the specified user data file.
        let users = load_users(&conf.users)?;
        let ssl_acceptor = if let Ok(identity) = conf.get_ssl_keys() {
            if conf.imap_ssl_port != 0 || conf.lmtp_ssl_port != 0 {
                match SslAcceptorBuilder::mozilla_intermediate(
                        SslMethod::tls(), &identity.pkey, 
                        &identity.cert, &identity.chain) {
                    Ok(a) => Some(a.build()),
                    _ => None
                }
            } else {
                None
            }
        } else {
            None
        };

        Ok(Server {
            conf: conf,
            users: users,
            ssl_acceptor: ssl_acceptor,
        })
    }

    /// Create a TCP listener on the server host and input port
    fn generic_listener(&self, port: u16) -> Option<Result<TcpListener>> {
        if port != 0 {
            Some(TcpListener::bind((&self.conf.host[..], port)))
        } else {
            None
        }
    }

    /// Create a TCP listener on the server host and imap port
    pub fn imap_listener(&self) -> Option<Result<TcpListener>> {
        self.generic_listener(self.conf.imap_port)
    }

    /// Create a TCP listener on the server host and imap ssl port
    pub fn imap_ssl_listener(&self) -> Option<Result<TcpListener>> {
        self.generic_listener(self.conf.imap_ssl_port)
    }

    /// Create a TCP listener on the server host and lmtp port
    pub fn lmtp_listener(&self) -> Option<Result<TcpListener>> {
        self.generic_listener(self.conf.lmtp_port)
    }

    /// Create a TCP listener on the server host and lmtp ssl port
    pub fn lmtp_ssl_listener(&self) -> Option<Result<TcpListener>> {
        self.generic_listener(self.conf.lmtp_ssl_port)
    }

    pub fn imap_ssl(&self, stream: TcpStream) -> Stream {
        if let Ok(addr) = stream.local_addr() {
            if addr.port() == self.conf.imap_ssl_port {
                if let Some(ref ssl_acceptor) = self.ssl_acceptor {
                    return Stream::Ssl(ssl_acceptor.accept(stream).unwrap());
                } else {
                    error!("Listening on SSL port without SSL certificate configured.");
                    let _ = stream.shutdown(Shutdown::Both);
                }
            }
        }
        Stream::Tcp(stream)
    }

    pub fn can_starttls(&self) -> bool {
        if let Some(_) = self.ssl_acceptor {
            true
        } else {
            false
        }
    }

    pub fn starttls(&self, stream: TcpStream) -> Option<SslStream<TcpStream>> {
        if let Some(ref ssl_acceptor) = self.ssl_acceptor {
            Some(ssl_acceptor.accept(stream).unwrap())
        } else {
            None
        }
    }

    fn host(&self) -> &String {
        &self.conf.host
    }

    pub fn get_user(&self, email: &Email) -> Option<&User> {
        self.users.get(email)
    }
}

pub fn lmtp_serve(serv: Arc<Server>, stream: TcpStream) {
    lmtp::serve(serv, BufStream::new(stream))
}
