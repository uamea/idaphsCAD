#[macro_export]
macro_rules! bind_callbacks {
    ($ui:expr, $sender:expr, {
        $(
            // 引数あり・なしを統合してパターンマッチ
            $callback:ident $( ( $($param:ident),* ) )? =>
                $( @$system:ident )? $message:expr
        ),* $(,)?
    }) => {
        $(
            let s = $sender.clone();
            $ui.$callback(move | $( $($param),* )? | {
                // @system があればそのまま、なければ AppMsg で包む
                let msg = bind_callbacks!(@internal $(@$system)? $message);
                let _ = s.try_send(msg);
            });
        )*
    };

    // 内部補助用：システムメッセージかアプリメッセージかを判定
    (@internal @system $msg:expr) => { $msg };
    (@internal $msg:expr) => { ControlMessage::AppMsg($msg) };
}
