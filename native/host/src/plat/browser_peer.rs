//! Linux IPv4 loopback authentication. The kernel's inet_diag response identifies
//! the owner of the connecting socket. Host root is the trusted principal.
use std::{
    io, mem,
    net::{Ipv4Addr, SocketAddr},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    os::unix::fs::MetadataExt,
};
use tokio::net::TcpStream;

const SOCK_DIAG_BY_FAMILY: u16 = 20;
const ESTABLISHED: u8 = 1;

pub fn host_namespaces() -> bool {
    // Linux reserves these nsfs inode numbers for the initial namespaces.
    [
        ("user", 4026531837),
        ("pid", 4026531836),
        ("net", 4026531833),
    ]
    .iter()
    .all(|(name, ino)| {
        std::fs::metadata(format!("/proc/thread-self/ns/{name}")).is_ok_and(|m| m.ino() == *ino)
    })
}

fn peer_uid(local: SocketAddr, remote: SocketAddr) -> io::Result<u32> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "unverified loopback peer");
    if local.ip() != Ipv4Addr::LOCALHOST || remote.ip() != Ipv4Addr::LOCALHOST {
        return Err(invalid());
    }
    // A fresh connected netlink socket per lookup isolates replies. Verify the sender
    // sockaddr as well as the echoed sequence, tuple, state and inode.
    let raw = unsafe {
        libc::socket(
            libc::AF_NETLINK,
            libc::SOCK_RAW | libc::SOCK_CLOEXEC,
            libc::NETLINK_SOCK_DIAG,
        )
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    let socket = unsafe { OwnedFd::from_raw_fd(raw) };
    let fd = socket.as_raw_fd();
    let mut kernel: libc::sockaddr_nl = unsafe { mem::zeroed() };
    kernel.nl_family = libc::AF_NETLINK as _;
    let size = mem::size_of_val(&kernel) as libc::socklen_t;
    if unsafe { libc::connect(fd, (&kernel as *const libc::sockaddr_nl).cast(), size) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let timeout = libc::timeval {
        tv_sec: 0,
        tv_usec: 200_000,
    };
    if unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_RCVTIMEO,
            (&timeout as *const libc::timeval).cast(),
            mem::size_of_val(&timeout) as _,
        )
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    // nlmsghdr (16) followed by inet_diag_req_v2 (56); ports and addresses are network order.
    let mut request = [0u8; 72];
    request[..4].copy_from_slice(&72u32.to_ne_bytes());
    request[4..6].copy_from_slice(&SOCK_DIAG_BY_FAMILY.to_ne_bytes());
    request[6..8].copy_from_slice(&(libc::NLM_F_REQUEST as u16).to_ne_bytes());
    request[8..12].copy_from_slice(&1u32.to_ne_bytes());
    request[16] = libc::AF_INET as u8;
    request[17] = libc::IPPROTO_TCP as u8;
    request[20..24].copy_from_slice(&(1u32 << ESTABLISHED).to_ne_bytes());
    request[24..26].copy_from_slice(&remote.port().to_be_bytes());
    request[26..28].copy_from_slice(&local.port().to_be_bytes());
    request[28..32].copy_from_slice(&Ipv4Addr::LOCALHOST.octets());
    request[44..48].copy_from_slice(&Ipv4Addr::LOCALHOST.octets());
    request[64..72].fill(0xff); // INET_DIAG_NOCOOKIE
    if unsafe { libc::send(fd, request.as_ptr().cast(), request.len(), 0) }
        != request.len() as isize
    {
        return Err(io::Error::last_os_error());
    }
    let mut reply = [0u8; 4096];
    let mut sender: libc::sockaddr_nl = unsafe { mem::zeroed() };
    let mut sender_size = size;
    let n = unsafe {
        libc::recvfrom(
            fd,
            reply.as_mut_ptr().cast(),
            reply.len(),
            libc::MSG_TRUNC,
            (&mut sender as *mut libc::sockaddr_nl).cast(),
            &mut sender_size,
        )
    };
    if n < 0 {
        return Err(io::Error::last_os_error());
    }
    // inet_diag_msg: family/state, sockid, five u32 fields (last two: UID and inode).
    if sender_size != size
        || sender.nl_family != kernel.nl_family
        || sender.nl_pid != 0
        || sender.nl_groups != 0
        || n < 88
        || n as usize > reply.len()
        || u32::from_ne_bytes(reply[..4].try_into().unwrap()) != n as u32
        || reply[4..6] != request[4..6]
        || reply[8..12] != request[8..12]
        || reply[16] != request[16]
        || reply[17] != ESTABLISHED
        || reply[20..56] != request[24..60]
        || reply[84..88] == [0; 4]
    {
        return Err(invalid());
    }
    Ok(u32::from_ne_bytes(reply[80..84].try_into().unwrap()))
}

pub fn accepts(stream: &TcpStream) -> bool {
    if !host_namespaces() {
        return false;
    }
    let (Ok(local), Ok(remote)) = (stream.local_addr(), stream.peer_addr()) else {
        return false;
    };
    if peer_uid(local, remote).ok() != Some(0) {
        return false;
    }
    // Recheck this exact accepted socket after the table lookup. An old socket
    // whose four-tuple was recycled cannot return to ESTABLISHED.
    still_established(stream)
}

fn still_established(stream: &TcpStream) -> bool {
    let mut info: libc::tcp_info = unsafe { mem::zeroed() };
    let mut len = mem::size_of_val(&info) as libc::socklen_t;
    let rc = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::IPPROTO_TCP,
            libc::TCP_INFO,
            (&mut info as *mut libc::tcp_info).cast(),
            &mut len,
        )
    };
    rc == 0 && len >= 1 && info.tcpi_state == ESTABLISHED
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn kernel_reports_live_peer_uid_and_rejects_torn_down_connections() {
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        for _ in 0..64 {
            let client = TcpStream::connect(listener.local_addr().unwrap())
                .await
                .unwrap();
            let (accepted, _) = listener.accept().await.unwrap();
            let uid = unsafe { libc::geteuid() };
            assert_eq!(
                peer_uid(
                    accepted.local_addr().unwrap(),
                    accepted.peer_addr().unwrap()
                )
                .unwrap(),
                uid
            );
            assert_eq!(accepts(&accepted), host_namespaces() && uid == 0);
            drop(client);
            accepted.readable().await.unwrap();
            assert!(!accepts(&accepted));
        }
    }

    #[tokio::test]
    async fn reused_tuple_does_not_reauthenticate_the_old_socket() {
        use tokio::net::TcpSocket;
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let target = listener.local_addr().unwrap();
        let mut source = SocketAddr::from((Ipv4Addr::LOCALHOST, 0));
        let mut stale = None;
        for _ in 0..64 {
            // Under load the kernel may not yet have released the previous tuple
            // after the reset. Retry only that transient connect failure, always
            // binding the same endpoint and retaining the previous descriptor.
            let client = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                loop {
                    let socket = TcpSocket::new_v4()?;
                    socket.set_reuseaddr(true)?;
                    socket.set_linger(Some(std::time::Duration::ZERO))?;
                    socket.bind(source)?;
                    match socket.connect(target).await {
                        Err(e) if e.kind() == io::ErrorKind::AddrNotAvailable => {
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                        }
                        result => return result,
                    }
                }
            })
            .await
            .expect("the exact tuple becomes reusable")
            .unwrap();
            source = client.local_addr().unwrap();
            let (current, _) = listener.accept().await.unwrap();
            assert_eq!(peer_uid(target, source).unwrap(), unsafe {
                libc::geteuid()
            });
            assert!(still_established(&current));
            if let Some(old) = stale.take() {
                assert_eq!(current.peer_addr().unwrap(), source);
                assert!(!still_established(&old));
                assert!(!accepts(&old));
            }
            drop(client);
            current.readable().await.unwrap();
            stale = Some(current);
        }
    }
}
