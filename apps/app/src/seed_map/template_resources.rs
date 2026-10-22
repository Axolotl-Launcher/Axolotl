pub(super) fn ancient_city_resources(
    version: &str,
) -> Option<(&'static [u8], &'static [u8])> {
    match version {
        "1.19.2" => Some((
            include_bytes!(
                "assets/versions/1.19.2/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.19.2/ancient-city-blocks.json.gz"
            ),
        )),
        "1.19.4" => Some((
            include_bytes!(
                "assets/versions/1.19.4/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.19.4/ancient-city-blocks.json.gz"
            ),
        )),
        "1.20" => Some((
            include_bytes!(
                "assets/versions/1.20.6/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.20.6/ancient-city-blocks.json.gz"
            ),
        )),
        "1.20.6" => Some((
            include_bytes!(
                "assets/versions/1.20.6/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.20.6/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.1" => Some((
            include_bytes!(
                "assets/versions/1.21.1/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.1/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.11" => Some((
            include_bytes!(
                "assets/versions/1.21.11/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.11/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.3" => Some((
            include_bytes!(
                "assets/versions/1.21.3/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.3/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.4" => Some((
            include_bytes!(
                "assets/versions/1.21.4/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.4/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.5" => Some((
            include_bytes!(
                "assets/versions/1.21.5/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.5/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.6" => Some((
            include_bytes!(
                "assets/versions/1.21.6/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.6/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.7" => Some((
            include_bytes!(
                "assets/versions/1.21.7/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.7/ancient-city-blocks.json.gz"
            ),
        )),
        "1.21.9" => Some((
            include_bytes!(
                "assets/versions/1.21.9/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/1.21.9/ancient-city-blocks.json.gz"
            ),
        )),
        "26.1" => Some((
            include_bytes!("assets/versions/26.1/ancient-city-layout.json.gz"),
            include_bytes!("assets/versions/26.1/ancient-city-blocks.json.gz"),
        )),
        "26.1.2" => Some((
            include_bytes!(
                "assets/versions/26.1.2/ancient-city-layout.json.gz"
            ),
            include_bytes!(
                "assets/versions/26.1.2/ancient-city-blocks.json.gz"
            ),
        )),
        "26.2" => Some((
            include_bytes!("assets/versions/26.2/ancient-city-layout.json.gz"),
            include_bytes!("assets/versions/26.2/ancient-city-blocks.json.gz"),
        )),
        "26.3" => Some((
            include_bytes!("assets/versions/26.3/ancient-city-layout.json.gz"),
            include_bytes!("assets/versions/26.3/ancient-city-blocks.json.gz"),
        )),
        _ => None,
    }
}
