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
                            <img src="data:image/png;base64,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" alt="Karna" width="28" height="28" style="vertical-align:middle;margin-right:8px;" />
                            <span style="font-size:20px;font-weight:700;color:#f9fafb;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;letter-spacing:-0.02em;vertical-align:middle;">karna</span>
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
