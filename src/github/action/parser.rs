use anyhow::{anyhow, bail, Result};
use rocket::http::Status;
use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

use crate::github::{client, data::*};

#[derive(Debug)]
pub struct Actionee(pub String);

impl FromStr for Actionee {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.get(..1) {
            Some("@") => match s.get(1..) {
                Some(user @ "williambotman") => Ok(Actionee(user.to_owned())),
                Some(user) => bail!("{} is not an allowed user.", user),
                None => bail!("{} is not a valid mention.", s),
            },
            Some(_) | None => bail!("{} is not a valid mention.", s),
        }
    }
}

#[derive(Debug)]
pub struct AuthorizedUser(pub String);

impl FromStr for AuthorizedUser {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            user @ ("williamboman" | "Conarius" | "mehalter" | "chrisgrieser") => Ok(AuthorizedUser(user.to_owned())),
            user => bail!("{} is not an allowed user.", user),
        }
    }
}

impl TryFrom<&GitHubUser> for AuthorizedUser {
    type Error = anyhow::Error;

    fn try_from(value: &GitHubUser) -> Result<Self, Self::Error> {
        value.login.parse()
    }
}

#[derive(Debug)]
pub struct RawCommand {
    pub raw_command: String,
    pub raw_arguments: Option<String>,
}

impl FromStr for RawCommand {
    type Err = anyhow::Error;

    fn from_str(command_body: &str) -> Result<Self, Self::Err> {
        match command_body.get(..1) {
            Some("/") => match command_body.get(1..) {
                Some(body) => match body.split_once(char::is_whitespace) {
                    Some((raw_command, raw_arguments)) => Ok(Self {
                        raw_command: (*raw_command).to_owned(),
                        raw_arguments: Some((*raw_arguments).to_owned()),
                    }),
                    None => Ok(Self {
                        raw_command: (&command_body[1..]).to_owned(),
                        raw_arguments: None,
                    }),
                },

                None => bail!("{} is not a valid command.", command_body),
            },
            Some(_) | None => bail!("{} is not a valid command", command_body),
        }
    }
}

#[async_trait]
pub trait AuthorizedActionContext: Sync + Send + Debug {
    async fn get_pull_request(&self) -> Result<Option<GitHubPullRequest>> {
        Ok(None)
    }

    fn get_repo(&self) -> &GitHubRepo;

    fn get_trigger(&self) -> &GitHubComment;
}

#[derive(Debug)]
pub struct Action<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    pub actionee: Actionee,
    pub command: Command,
}

impl<Command> FromStr for Action<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        let (mention, command) = s
            .split_once(" ")
            .ok_or_else(|| anyhow!("{} is not valid action syntax.", s))?;

        let actionee = mention.parse()?;
        let command = command.parse::<RawCommand>()?.try_into()?;

        Ok(Self { actionee, command })
    }
}

impl<Command> TryFrom<&GitHubPullRequestReviewComment> for Action<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    type Error = anyhow::Error;

    fn try_from(value: &GitHubPullRequestReviewComment) -> Result<Self, Self::Error> {
        value
            .comment
            .body
            .as_ref()
            .ok_or_else(|| anyhow!("Body is empty."))?
            .parse()
    }
}

impl<Command> TryFrom<&GitHubPullRequestReview> for Action<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    type Error = anyhow::Error;

    fn try_from(value: &GitHubPullRequestReview) -> Result<Self, Self::Error> {
        value
            .review
            .body
            .as_ref()
            .ok_or_else(|| anyhow!("Body is empty."))?
            .parse()
    }
}

#[async_trait]
pub trait AuthorizedActionExecutor: TryFrom<RawCommand, Error = anyhow::Error> {
    async fn execute(
        action: AuthorizedAction<Self>,
    ) -> Result<Box<dyn Display + Send>, (Status, anyhow::Error)>;
}

#[derive(Debug)]
pub struct AuthorizedAction<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    pub action: Action<Command>,
    pub context: Box<dyn AuthorizedActionContext>,
    pub authorized_by: AuthorizedUser,
}

impl<Command> TryFrom<GitHubPullRequestReviewComment> for AuthorizedAction<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    type Error = anyhow::Error;

    fn try_from(value: GitHubPullRequestReviewComment) -> Result<Self, Self::Error> {
        let authorized_by = (&value.comment.user).try_into()?;
        Ok(Self {
            action: (&value).try_into()?,
            context: Box::new(value),
            authorized_by,
        })
    }
}

impl<Command> TryFrom<GitHubPullRequestReview> for AuthorizedAction<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    type Error = anyhow::Error;

    fn try_from(value: GitHubPullRequestReview) -> Result<Self, Self::Error> {
        let authorized_by = (&value.review.user).try_into()?;
        Ok(Self {
            action: (&value).try_into()?,
            context: Box::new(value),
            authorized_by,
        })
    }
}

#[async_trait]
impl AuthorizedActionContext for GitHubPullRequestReviewComment {
    async fn get_pull_request(&self) -> Result<Option<GitHubPullRequest>> {
        Ok(Some(self.pull_request.clone()))
    }

    fn get_trigger(&self) -> &GitHubComment {
        &self.comment
    }

    fn get_repo(&self) -> &GitHubRepo {
        &self.pull_request.base.repo
    }
}

#[async_trait]
impl AuthorizedActionContext for GitHubPullRequestReview {
    async fn get_pull_request(&self) -> Result<Option<GitHubPullRequest>> {
        Ok(Some(self.pull_request.clone()))
    }

    fn get_trigger(&self) -> &GitHubComment {
        &self.review
    }

    fn get_repo(&self) -> &GitHubRepo {
        &self.pull_request.base.repo
    }
}

impl<Command> TryFrom<&GitHubIssueCommentEvent> for Action<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    type Error = anyhow::Error;

    fn try_from(value: &GitHubIssueCommentEvent) -> Result<Self, Self::Error> {
        value
            .comment
            .body
            .as_ref()
            .ok_or_else(|| anyhow!("Body is empty."))?
            .parse()
    }
}

impl<Command> TryFrom<GitHubIssueCommentEvent> for AuthorizedAction<Command>
where
    Command: TryFrom<RawCommand, Error = anyhow::Error>,
{
    type Error = anyhow::Error;

    fn try_from(value: GitHubIssueCommentEvent) -> Result<Self, Self::Error> {
        let authorized_by = (&value.comment.user).try_into()?;
        Ok(Self {
            action: (&value).try_into()?,
            context: Box::new(value),
            authorized_by,
        })
    }
}

#[async_trait]
impl AuthorizedActionContext for GitHubIssueCommentEvent {
    async fn get_pull_request(&self) -> Result<Option<GitHubPullRequest>> {
        if let Some(pr) = self.issue.pull_request.as_ref() {
            Ok(client::get(&pr.url).await?.json().await?)
        } else {
            Ok(None)
        }
    }

    fn get_trigger(&self) -> &GitHubComment {
        &self.comment
    }

    fn get_repo(&self) -> &GitHubRepo {
        &self.repository
    }
}
