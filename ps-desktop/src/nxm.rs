use ps_server::services::nexus::links::NexusLinks;

pub fn nxm_arguments<I, S>(args: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    args.into_iter()
        .map(|arg| arg.as_ref().trim().to_string())
        .filter(|arg| {
            arg.get(..6)
                .is_some_and(|scheme| scheme.eq_ignore_ascii_case("nxm://"))
        })
        .collect()
}

pub fn deliver<I, S>(links: &NexusLinks, args: I) -> usize
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let found = nxm_arguments(args);
    for link in &found {
        links.push_raw(link);
    }
    found.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_nxm_arguments_are_kept() {
        let args = [
            "C:\\Program Files\\PalStudio\\palstudio.exe",
            "--flag",
            " NXM://palworld/mods/1/files/2 ",
            "nxm:",
            "ééé",
        ];
        assert_eq!(
            nxm_arguments(args),
            vec!["NXM://palworld/mods/1/files/2".to_string()]
        );
    }

    #[test]
    fn delivered_links_wait_in_the_inbox() {
        let links = NexusLinks::default();
        let delivered = deliver(
            &links,
            vec![
                "palstudio".to_string(),
                "nxm://palworld/mods/4821/files/99001".to_string(),
            ],
        );
        assert_eq!(delivered, 1);
        assert_eq!(links.pending_len(), 1);
    }
}
