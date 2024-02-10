use crate::{serenity, Context, Result};

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
