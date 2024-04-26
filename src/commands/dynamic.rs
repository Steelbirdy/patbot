use crate::prelude::*;

type Command = poise::Command<crate::Data, crate::Error>;

#[poise::command(slash_command, subcommand_required, subcommands("create", "delete"))]
pub async fn reply(_ctx: ApplicationContext<'_>) -> Result {
    unreachable!()
}

/// Create a new Patbot command
#[poise::command(slash_command)]
pub async fn create(ctx: ApplicationContext<'_>) -> Result {
    fn command_exists(ctx: ApplicationContext<'_>, name: &str) -> bool {
        let dynamic_command_exists = ctx
            .data()
            .use_reply_commands(|commands| commands.names().any(|cmd| cmd == name));
        let static_command_exists = ctx
            .framework()
            .options()
            .commands
            .iter()
            .any(|cmd| cmd.name == name);
        dynamic_command_exists || static_command_exists
    }

    #[derive(poise::Modal)]
    #[name = "Create a new reply command"]
    struct CreateModal {
        #[name = "Command Name"]
        #[placeholder = "The name of the command"]
        #[max_length = 20]
        command_name: String,
        #[name = "Command Description"]
        #[placeholder = "A description of the command"]
        #[max_length = 100]
        command_description: Option<String>,
        #[name = "Response Text (optional)"]
        #[placeholder = "The text Patbot uses in the response"]
        #[paragraph]
        response_text: Option<String>,
        #[name = "Response Attachment URL (optional)"]
        #[placeholder = "https://your.attachment.here"]
        response_attachment_url: Option<String>,
    }

    let Some(CreateModal {
        command_name,
        command_description,
        response_text,
        response_attachment_url,
    }) = poise::Modal::execute(ctx).await?
    else {
        return Ok(());
    };

    if command_name.contains(|ch: char| ch.is_whitespace()) {
        reply_error!(ctx, "The command name cannot have any spaces in it.");
    }
    if command_exists(ctx, &command_name) {
        reply_error!(ctx, "A command named `{}` already exists.", command_name);
    }

    if response_text.is_none() && response_attachment_url.is_none() {
        reply_error!(
            ctx,
            "You must provide at least one of 'Response Text' or 'Response Attachment URL'"
        );
    }

    let response = ReplyCommandResponse {
        content: response_text,
        attachment_url: response_attachment_url,
    };

    let owner = ctx.author().id;

    let command = ReplyCommand {
        ids: Vec::new(),
        name: command_name.clone(),
        description: command_description,
        owner: owner.get(),
        response,
    };

    ctx.data()
        .register_reply_command(ctx.into(), command)
        .await?;

    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                ":white_check_mark:  created command `{command_name}`."
            ))
            .reply(true),
    )
    .await?;

    Ok(())
}

/// Delete a Patbot command that you created
#[poise::command(slash_command)]
pub async fn delete(
    ctx: ApplicationContext<'_>,
    #[description = "The name of the command"]
    #[autocomplete = "autocomplete_delete_param_command"]
    command: String,
) -> Result {
    let author_is_owner = ctx
        .data()
        .use_reply_commands(|commands| commands.get(&command).map(|cmd| cmd.author_is_owner(ctx)));
    match author_is_owner {
        None => reply_error!(ctx, "That command does not exist."),
        Some(false) => reply_error!(ctx, "Only the creator of the command can delete it."),
        Some(true) => {}
    }

    let command_existed_and_was_deleted = ctx
        .data()
        .delete_reply_command(ctx.into(), &command)
        .await?;
    assert!(command_existed_and_was_deleted);
    ctx.reply(format!(
        ":white_check_mark: The command `{command}` was deleted."
    ))
    .await?;

    Ok(())
}

async fn autocomplete_delete_param_command<'a>(
    ctx: ApplicationContext<'_>,
    partial: &'a str,
) -> impl Iterator<Item = String> + 'a {
    let options: Vec<_> = ctx
        .data()
        .use_reply_commands(|commands| commands.names().map(ToString::to_string).collect());
    options
        .into_iter()
        .filter(move |name| name.starts_with(partial))
}

/// We use u64 instead of the relevant serenity::*Id because they don't deserialize correctly
///  when using `shuttle_persist`.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplyCommand {
    pub ids: Vec<(u64, u64)>,
    pub name: String,
    pub description: Option<String>,
    pub owner: u64,
    pub response: ReplyCommandResponse,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ReplyCommandResponse {
    content: Option<String>,
    attachment_url: Option<String>,
}

impl ReplyCommandResponse {
    pub async fn into_serenity_response(
        self,
        http: impl AsRef<serenity::Http>,
    ) -> Result<serenity::CreateInteractionResponseMessage> {
        let ReplyCommandResponse {
            content,
            attachment_url,
        } = self;
        let mut ret = serenity::CreateInteractionResponseMessage::default();
        if let Some(content) = content {
            ret = ret.content(content);
        }
        if let Some(url) = attachment_url {
            let attachment = serenity::CreateAttachment::url(http, &url).await?;
            ret = ret.add_file(attachment);
        }
        Ok(ret)
    }
}

impl ReplyCommand {
    pub fn author_is_owner(&self, ctx: ApplicationContext<'_>) -> bool {
        let user = ctx.author().id;
        self.owner == user.get() || ctx.framework.options.owners.contains(&user)
    }

    pub fn to_poise_command(&self) -> Command {
        async fn inner(ctx: Context<'_>, name: &str) -> Result {
            let response = ctx.data().reply_command_response(name).unwrap();
            let response = response.into_serenity_response(ctx).await?;
            let Context::Application(ctx) = ctx else {
                unreachable!()
            };
            ctx.interaction
                .create_response(ctx, serenity::CreateInteractionResponse::Message(response))
                .await?;
            Ok(())
        }

        let name = self.name.clone();
        Command {
            prefix_action: None,
            slash_action: Some(|ctx: ApplicationContext<'_>| {
                Box::pin(async move {
                    let command_name = &ctx.interaction.data.name;
                    inner(ctx.into(), command_name)
                        .await
                        .map_err(|error| poise::FrameworkError::new_command(ctx.into(), error))
                })
            }),
            context_menu_action: None,
            subcommands: Vec::new(),
            subcommand_required: false,
            name: name.clone(),
            qualified_name: name.clone(),
            identifying_name: name.clone(),
            source_code_name: name.clone(),
            description: self.description.clone(),
            guild_only: true,
            ..Default::default()
        }
    }
}
