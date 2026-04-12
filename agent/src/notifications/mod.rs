use anyhow::Result;
use tracing::{info, warn};

use crate::config::Config;
use crate::models::AgentTask;

pub async fn send_plan_ready(config: &Config, task: &AgentTask) -> Result<()> {
    let subject = format!("Plan ready: {}", task.title);
    let plan_preview = task
        .plan_content
        .as_deref()
        .unwrap_or("No plan content")
        .chars()
        .take(500)
        .collect::<String>();

    let html = render_email(
        "Plan Ready for Review",
        "#3b82f6",
        &task.title,
        task.repo.as_deref().unwrap_or("(multiple)"),
        &format!(
            r#"<p style="margin:0 0 12px;color:#d1d5db;font-size:14px;">A plan has been generated and is waiting for your review.</p>
            <div style="background:#1e1e2e;border:1px solid #374151;border-radius:6px;padding:12px 16px;margin-top:8px;">
                <p style="margin:0;color:#9ca3af;font-size:13px;font-family:'SFMono-Regular',Consolas,monospace;white-space:pre-wrap;word-break:break-word;">{}</p>
            </div>"#,
            html_escape(&plan_preview),
        ),
        None,
    );

    send_email(config, &subject, &html).await
}

pub async fn send_pr_opened(config: &Config, task: &AgentTask) -> Result<()> {
    let pr_url = task.pr_url.as_deref().unwrap_or("#");
    let subject = format!("PR opened: {}", task.title);
    let html = render_email(
        "Pull Request Opened",
        "#a78bfa",
        &task.title,
        task.repo.as_deref().unwrap_or("(multiple)"),
        r#"<p style="margin:0;color:#d1d5db;font-size:14px;">The agent has opened a pull request for your review.</p>"#,
        Some((pr_url, "View Pull Request")),
    );

    send_email(config, &subject, &html).await
}

pub async fn send_task_failed(config: &Config, task: &AgentTask) -> Result<()> {
    let error = task.error_message.as_deref().unwrap_or("Unknown error");
    let subject = format!("Agent failed: {}", task.title);
    let html = render_email(
        "Task Failed",
        "#ef4444",
        &task.title,
        task.repo.as_deref().unwrap_or("(multiple)"),
        &format!(
            r#"<p style="margin:0 0 12px;color:#d1d5db;font-size:14px;">The agent encountered an error while working on this task.</p>
            <div style="background:#1e1e2e;border:1px solid #7f1d1d;border-radius:6px;padding:12px 16px;margin-top:8px;">
                <p style="margin:0;color:#fca5a5;font-size:13px;font-family:'SFMono-Regular',Consolas,monospace;white-space:pre-wrap;word-break:break-word;">{}</p>
            </div>"#,
            html_escape(error),
        ),
        None,
    );

    send_email(config, &subject, &html).await
}

pub async fn send_task_done(config: &Config, task: &AgentTask) -> Result<()> {
    let pr_url = task.pr_url.as_deref().unwrap_or("#");
    let subject = format!("Task completed: {}", task.title);
    let html = render_email(
        "Task Completed",
        "#22c55e",
        &task.title,
        task.repo.as_deref().unwrap_or("(multiple)"),
        r#"<p style="margin:0;color:#d1d5db;font-size:14px;">The agent has finished this task. The pull request is ready for your final review.</p>"#,
        Some((pr_url, "View Pull Request")),
    );

    send_email(config, &subject, &html).await
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_email(
    heading: &str,
    accent: &str,
    task_title: &str,
    repo: &str,
    body_content: &str,
    cta: Option<(&str, &str)>,
) -> String {
    let cta_block = match cta {
        Some((url, label)) => format!(
            r#"<tr><td style="padding:24px 0 0;">
                <a href="{url}" style="display:inline-block;background:{accent};color:#0f0f17;font-size:14px;font-weight:600;text-decoration:none;padding:10px 24px;border-radius:6px;" target="_blank">{label}</a>
            </td></tr>"#,
        ),
        None => String::new(),
    };

    format!(
        r##"<!DOCTYPE html>
<html lang="en" xmlns="http://www.w3.org/1999/xhtml" xmlns:v="urn:schemas-microsoft-com:vml" xmlns:o="urn:schemas-microsoft-com:office:office">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1">
    <meta http-equiv="X-UA-Compatible" content="IE=edge">
    <meta name="x-apple-disable-message-reformatting">
    <meta name="format-detection" content="telephone=no,address=no,email=no,date=no,url=no">
    <title>{heading}</title>
    <!--[if mso]><noscript><xml><o:OfficeDocumentSettings><o:PixelsPerInch>96</o:PixelsPerInch></o:OfficeDocumentSettings></xml></noscript><![endif]-->
    <!--[if mso]><style>table,td,div,p,a,span{{font-family:Arial,Helvetica,sans-serif!important;}}</style><![endif]-->
</head>
<body style="margin:0;padding:0;background-color:#0f0f17;-webkit-text-size-adjust:none;text-size-adjust:none;">
    <!-- Preheader (hidden text for inbox preview) -->
    <div style="display:none;font-size:1px;color:#0f0f17;line-height:1px;max-height:0;max-width:0;opacity:0;overflow:hidden;">
        {heading} — {task_title} ({repo})
    </div>

    <table role="presentation" cellspacing="0" cellpadding="0" border="0" width="100%" style="background-color:#0f0f17;">
        <tr>
            <td align="center" style="padding:32px 16px;">
                <table role="presentation" cellspacing="0" cellpadding="0" border="0" width="560" style="max-width:560px;width:100%;">

                    <!-- Logo -->
                    <tr>
                        <td style="padding:0 0 24px;">
                            <span style="font-size:20px;font-weight:700;color:#f9fafb;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;letter-spacing:-0.02em;">karna</span>
                        </td>
                    </tr>

                    <!-- Card -->
                    <tr>
                        <td style="background-color:#18181b;border:1px solid #27272a;border-radius:12px;overflow:hidden;">
                            <!-- Accent bar -->
                            <div style="height:3px;background:{accent};"></div>

                            <table role="presentation" cellspacing="0" cellpadding="0" border="0" width="100%">
                                <!-- Heading -->
                                <tr>
                                    <td style="padding:24px 24px 0;">
                                        <h1 style="margin:0;font-size:20px;font-weight:600;color:#f9fafb;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">{heading}</h1>
                                    </td>
                                </tr>

                                <!-- Task meta -->
                                <tr>
                                    <td style="padding:16px 24px 0;">
                                        <table role="presentation" cellspacing="0" cellpadding="0" border="0">
                                            <tr>
                                                <td style="padding-right:16px;">
                                                    <p style="margin:0 0 2px;font-size:11px;font-weight:500;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">Task</p>
                                                    <p style="margin:0;font-size:14px;color:#e5e7eb;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">{task_title}</p>
                                                </td>
                                                <td>
                                                    <p style="margin:0 0 2px;font-size:11px;font-weight:500;text-transform:uppercase;letter-spacing:0.05em;color:#6b7280;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">Repository</p>
                                                    <p style="margin:0;font-size:14px;color:#e5e7eb;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;"><code style="background:#1e1e2e;padding:2px 6px;border-radius:4px;font-size:13px;">{repo}</code></p>
                                                </td>
                                            </tr>
                                        </table>
                                    </td>
                                </tr>

                                <!-- Divider -->
                                <tr>
                                    <td style="padding:16px 24px 0;">
                                        <div style="height:1px;background:#27272a;"></div>
                                    </td>
                                </tr>

                                <!-- Body content -->
                                <tr>
                                    <td style="padding:16px 24px 0;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
                                        {body_content}
                                    </td>
                                </tr>

                                <!-- CTA button -->
                                {cta_block}

                                <!-- Bottom padding -->
                                <tr>
                                    <td style="padding:24px;"></td>
                                </tr>
                            </table>
                        </td>
                    </tr>

                    <!-- Footer -->
                    <tr>
                        <td style="padding:24px 0 0;text-align:center;">
                            <p style="margin:0;font-size:12px;color:#4b5563;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;">
                                Sent by Karna &mdash; autonomous coding agent
                            </p>
                        </td>
                    </tr>

                </table>
            </td>
        </tr>
    </table>
</body>
</html>"##,
    )
}

async fn send_email(config: &Config, subject: &str, html: &str) -> Result<()> {
    let api_key = match &config.resend_api_key {
        Some(key) => key,
        None => {
            info!(subject, "Skipping email (RESEND_API_KEY not set)");
            return Ok(());
        }
    };

    let to = match &config.notification_email {
        Some(email) => email,
        None => {
            warn!("No notification email configured, skipping");
            return Ok(());
        }
    };

    let client = reqwest::Client::new();
    let res = client
        .post("https://api.resend.com/emails")
        .bearer_auth(api_key)
        .json(&serde_json::json!({
            "from": config.from_email,
            "to": [to],
            "subject": subject,
            "html": html,
        }))
        .send()
        .await?;

    if !res.status().is_success() {
        let body = res.text().await.unwrap_or_default();
        warn!(body, "Resend API error");
    } else {
        info!(to, subject, "Email sent");
    }

    Ok(())
}
