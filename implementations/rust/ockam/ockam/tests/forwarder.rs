use ockam::remote::{RemoteForwarder, RemoteForwarderTrustOptions};
use ockam::workers::Echoer;
use ockam::ForwardingService;
use ockam_core::sessions::{SessionPolicy, Sessions};
use ockam_core::{route, Address, AllowAll, Result};
use ockam_node::{Context, MessageReceiveOptions, MessageSendReceiveOptions};
use ockam_transport_tcp::{TcpConnectionTrustOptions, TcpListenerTrustOptions, TcpTransport};
use std::time::Duration;

// Node creates a Forwarding service and a Remote Forwarder, Echoer is reached through the Forwarder. No session
#[ockam_macros::test]
async fn test1(ctx: &mut Context) -> Result<()> {
    ForwardingService::create(ctx, "forwarding_service", AllowAll, AllowAll).await?;

    ctx.start_worker("echoer", Echoer, AllowAll, AllowAll)
        .await?;

    let remote_info =
        RemoteForwarder::create(ctx, route![], RemoteForwarderTrustOptions::new()).await?;

    let resp = ctx
        .send_and_receive_extended::<String>(
            route![remote_info.remote_address(), "echoer"],
            "Hello".to_string(),
            MessageSendReceiveOptions::new(),
        )
        .await?;

    assert_eq!(resp, "Hello");

    ctx.stop().await
}

// Cloud: Hosts a Forwarding service and listens on a tcp port. No session
// Server: Connects to a Cloud using tcp and creates a dynamic Forwarder. Using session
// Client: Connects to a Cloud using tcp and reaches to the Server's Echoer. Using session
#[ockam_macros::test]
async fn test2(ctx: &mut Context) -> Result<()> {
    ForwardingService::create(ctx, "forwarding_service", AllowAll, AllowAll).await?;
    let cloud_tcp = TcpTransport::create(ctx).await?;
    let (socket_addr, _) = cloud_tcp
        .listen("127.0.0.1:0", TcpListenerTrustOptions::new())
        .await?;

    let server_sessions = Sessions::default();
    let server_tcp_session_id = server_sessions.generate_session_id();

    ctx.start_worker("echoer", Echoer, AllowAll, AllowAll)
        .await?;
    server_sessions.add_consumer(
        &Address::from_string("echoer"),
        &server_tcp_session_id,
        SessionPolicy::ProducerAllowMultiple,
    );

    let server_tcp = TcpTransport::create(ctx).await?;
    let cloud_connection = server_tcp
        .connect(
            socket_addr.to_string(),
            TcpConnectionTrustOptions::new().as_producer(&server_sessions, &server_tcp_session_id),
        )
        .await?;

    let remote_info = RemoteForwarder::create(
        ctx,
        cloud_connection.clone(),
        RemoteForwarderTrustOptions::new()
            .as_consumer_and_producer(&server_sessions, &server_tcp_session_id),
    )
    .await?;

    let client_sessions = Sessions::default();
    let client_tcp_session_id = client_sessions.generate_session_id();

    let client_tcp = TcpTransport::create(ctx).await?;
    let cloud_connection = client_tcp
        .connect(
            socket_addr.to_string(),
            TcpConnectionTrustOptions::new().as_producer(&client_sessions, &client_tcp_session_id),
        )
        .await?;

    let resp = ctx
        .send_and_receive_extended::<String>(
            route![cloud_connection, remote_info.remote_address(), "echoer"],
            "Hello".to_string(),
            MessageSendReceiveOptions::new().with_session(
                &client_sessions,
                &client_tcp_session_id,
                SessionPolicy::ProducerAllowMultiple,
            ),
        )
        .await?;

    assert_eq!(resp, "Hello");

    ctx.stop().await
}

// Server: Connects to a Cloud using tcp and creates a dynamic Forwarder. Using session
// Cloud: Hosts a Forwarding service and sends a message to the Client with and without a session
#[ockam_macros::test]
async fn test3(ctx: &mut Context) -> Result<()> {
    ForwardingService::create(ctx, "forwarding_service", AllowAll, AllowAll).await?;
    let cloud_tcp = TcpTransport::create(ctx).await?;
    let (socket_addr, _) = cloud_tcp
        .listen("127.0.0.1:0", TcpListenerTrustOptions::new())
        .await?;

    let server_sessions = Sessions::default();
    let server_tcp_session_id = server_sessions.generate_session_id();

    let server_tcp = TcpTransport::create(ctx).await?;
    let cloud_connection = server_tcp
        .connect(
            socket_addr.to_string(),
            TcpConnectionTrustOptions::new().as_producer(&server_sessions, &server_tcp_session_id),
        )
        .await?;

    let remote_info = RemoteForwarder::create(
        ctx,
        cloud_connection.clone(),
        RemoteForwarderTrustOptions::new()
            .as_consumer_and_producer(&server_sessions, &server_tcp_session_id),
    )
    .await?;

    let mut child_ctx = ctx.new_detached("ctx", AllowAll, AllowAll).await?;
    ctx.send(
        route![remote_info.remote_address(), "ctx"],
        "Hello".to_string(),
    )
    .await?;

    let res = child_ctx
        .receive_extended::<String>(
            MessageReceiveOptions::new().with_timeout(Duration::from_millis(100)),
        )
        .await;

    assert!(res.is_err(), "Should not pass outgoing access control");

    ctx.send(
        route![remote_info.remote_address(), "ctx"],
        "Hello".to_string(),
    )
    .await?;

    server_sessions.add_consumer(
        &Address::from_string("ctx"),
        &server_tcp_session_id,
        SessionPolicy::ProducerAllowMultiple,
    );

    let res = child_ctx
        .receive_extended::<String>(
            MessageReceiveOptions::new().with_timeout(Duration::from_millis(100)),
        )
        .await?;

    assert_eq!(res.body(), "Hello");

    ctx.stop().await
}