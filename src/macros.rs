macro_rules! reply_error {
    ($ctx:ident, $error_message:literal $(, $args:expr)*) => {{
        let _ = $ctx.send(poise::CreateReply::default()
            .content(format!(concat!(":x: ", $error_message) $(, $args)*))
            .reply(true)
            .ephemeral(true))
            .await;
        return Ok(());
    }};
}

macro_rules! respond_to_interaction {
    ($ctx:ident, $interaction:ident) => {
        $interaction.create_response(
            $ctx,
            crate::serenity::CreateInteractionResponse::Acknowledge,
        )
    };
    ($ctx:ident, $interaction:ident, $message:expr) => {
        $interaction.create_response(
            $ctx,
            crate::serenity::CreateInteractionResponse::Message(
                crate::serenity::CreateInteractionResponseMessage::default()
                    .content($message)
                    .ephemeral(true),
            ),
        )
    };
}

pub(crate) use reply_error;
pub(crate) use respond_to_interaction;
