pub mod client;
pub mod discovery;
pub mod models;

/// Live tests against a real WordPress site. Run with:
/// `CREATEPRESS_TEST_SITE=… CREATEPRESS_TEST_USER=… CREATEPRESS_TEST_APP_PASSWORD=… cargo test -- --ignored`
#[cfg(test)]
mod live_tests {
    use super::client::WpClient;
    use super::discovery::discover;
    use super::models::{post_type_access, RestPost, RestPostType, RestUser};
    use crate::keychain::Credentials;
    use std::collections::HashMap;

    fn env() -> Option<(String, String, String)> {
        Some((
            std::env::var("CREATEPRESS_TEST_SITE").ok()?,
            std::env::var("CREATEPRESS_TEST_USER").ok()?,
            std::env::var("CREATEPRESS_TEST_APP_PASSWORD").ok()?,
        ))
    }

    #[tokio::test]
    #[ignore = "needs a live WordPress site"]
    async fn discovers_and_reads_as_the_user() {
        let Some((site, user, pw)) = env() else {
            return;
        };
        let local = site.starts_with("http://");
        let d = discover(&site, local).await.expect("discovery");
        assert!(d.supports_application_passwords);
        let client =
            WpClient::new(d.api_root.clone(), &Credentials::new(&user, &pw), local).unwrap();

        let (me, _) = client
            .get::<RestUser>("wp/v2/users/me", &[("context", "edit".into())])
            .await
            .expect("users/me");
        let (types, _) = client
            .get::<HashMap<String, RestPostType>>("wp/v2/types", &[("context", "edit".into())])
            .await
            .expect("types");
        let access: Vec<_> = types
            .values()
            .map(|t| post_type_access(t, &me.capabilities))
            .collect();
        println!(
            "site={} bridge={} roles={:?}",
            d.name, d.bridge_available, me.roles
        );
        for a in &access {
            println!(
                "  {} create={} publish={}",
                a.slug, a.can_create, a.can_publish
            );
        }
        let (posts, headers) = client
            .get::<Vec<RestPost>>(
                "wp/v2/posts",
                &[
                    ("context", "edit".into()),
                    ("status", "any".into()),
                    ("per_page", "5".into()),
                ],
            )
            .await
            .expect("posts");
        println!(
            "  posts={} total={:?}",
            posts.len(),
            headers.get("x-wp-total")
        );

        if d.bridge_available {
            #[derive(serde::Deserialize)]
            struct Handoff {
                url: String,
            }
            let target = d.admin_url.join("post-new.php").unwrap();
            let h: Handoff = client
                .post(
                    "createpress/v1/handoff",
                    &serde_json::json!({ "redirect_to": target.as_str() }),
                )
                .await
                .expect("handoff");
            let url = url::Url::parse(&h.url).unwrap();
            assert!(crate::security::same_origin(&d.admin_url, &url));
            assert!(url
                .query_pairs()
                .any(|(k, v)| k == "action" && v == "createpress_handoff"));
            println!("  handoff ok");
        }
    }

    #[tokio::test]
    #[ignore = "needs a live WordPress site"]
    async fn wrong_password_is_unauthorized() {
        let Some((site, user, _)) = env() else { return };
        let local = site.starts_with("http://");
        let d = discover(&site, local).await.unwrap();
        let client = WpClient::new(
            d.api_root,
            &Credentials::new(&user, "wrong wrong wrong"),
            local,
        )
        .unwrap();
        let err = client
            .get::<RestUser>("wp/v2/users/me", &[])
            .await
            .unwrap_err();
        assert!(
            matches!(err, crate::error::AppError::Unauthorized),
            "{err:?}"
        );
    }
}
