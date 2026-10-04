{
            let collapsed_daily_mode = !chrome.input_modes_expanded;
            if chrome.native_candidate_page.is_some() {
                include!("panel_scene_native_candidates_block.rs");
            } else {
                include!("panel_scene_next_tokens_block.rs");
                include!("panel_scene_sentence_candidates_block.rs");
            }
            include!("panel_scene_render_scene_tail.rs")

}
