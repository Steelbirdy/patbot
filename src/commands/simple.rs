use crate::prelude::*;

#[poise::command(slash_command)]
pub async fn gazoo(ctx: Context<'_>) -> Result {
    let attachment = serenity::CreateAttachment::url(ctx, "https://cdn.discordapp.com/attachments/625764441862045718/655904066341437450/gazoo.png?ex=65d28943&is=65c01443&hm=9f55237dcec69b117cbc23ead93826dc36f7968a58cfb5f7c37f8abe8477d494&").await?;
    ctx.send(
        poise::CreateReply::default()
            .reply(true)
            .content("***OOOOUWWUH***")
            .attachment(attachment),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command, subcommand_required, subcommands("create", "delete"))]
pub async fn reply(_ctx: ApplicationContext<'_>) -> Result {
    unreachable!()
}

#[poise::command(slash_command)]
pub async fn create(ctx: ApplicationContext<'_>) -> Result {
    #[derive(poise::Modal)]
    #[name = "Create a new reply command"]
    struct CreateModal {
        #[name = "Command Name"]
        #[placeholder = "my_command"]
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

    // TODO: do not allow existing commands to be overwritten

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
        name: command_name.clone(),
        description: command_description,
        owner,
        response,
    };

    ctx.data()
        .register_reply_command(ctx.into(), command)
        .await?;

    ctx.send(
        poise::CreateReply::default()
            .content(format!(
                ":white_check_mark: created command `{command_name}`."
            ))
            .reply(true),
    )
    .await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn delete(ctx: ApplicationContext<'_>) -> Result {
    ctx.reply("Not yet implemented.").await?;
    Ok(())
}

pub struct ReplyCommand {
    pub name: String,
    pub description: Option<String>,
    pub owner: serenity::UserId,
    pub response: ReplyCommandResponse,
}

#[derive(Clone)]
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
        self.owner == user || ctx.framework.options.owners.contains(&user)
    }

    pub fn to_poise_command(&self) -> crate::Command {
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
        crate::Command {
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
