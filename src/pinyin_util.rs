use pinyin::ToPinyin;

/// Compute full lowercase pinyin and first-letter initials for any string containing Chinese characters.
pub fn to_pinyin_keys(text: &str) -> (String, String) {
    let mut full = String::new();
    let mut initials = String::new();

    for ch in text.chars() {
        if let Some(py) = ch.to_pinyin() {
            let plain = py.plain();
            full.push_str(plain);
            if let Some(first) = plain.chars().next() {
                initials.push(first);
            }
        } else if ch.is_ascii_alphanumeric() {
            let lower = ch.to_ascii_lowercase();
            full.push(lower);
            initials.push(lower);
        }
    }

    (full, initials)
}

pub struct BilingualFxEntry {
    pub match_name: &'static str,
    pub en_name: &'static str,
    pub zh_name: &'static str,
    pub category: &'static str,
}

/// Curated bilingual dictionary of built-in After Effects effects.
/// Serves both as cross-language search enrichment (e.g. searching "blur" in Chinese AE or "高斯"/"gsmh" in English AE)
/// and as the initial out-of-the-box catalog before AE first syncs `app.effects`.
pub static BUILTIN_AE_EFFECTS: &[BilingualFxEntry] = &[
    // Blur & Sharpen
    BilingualFxEntry { match_name: "ADBE Gaussian Blur 2", en_name: "Gaussian Blur", zh_name: "高斯模糊", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "ADBE Box Blur2", en_name: "Fast Box Blur", zh_name: "快速方框模糊", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "ADBE Camera Lens Blur", en_name: "Camera Lens Blur", zh_name: "摄像机镜头模糊", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "ADBE Motion Blur", en_name: "Directional Blur", zh_name: "定向模糊", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "ADBE Radial Blur", en_name: "Radial Blur", zh_name: "径向模糊", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "CC Radial Fast Blur", en_name: "CC Radial Fast Blur", zh_name: "CC 径向快速模糊", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "CC Vector Blur", en_name: "CC Vector Blur", zh_name: "CC 矢量模糊", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "ADBE Sharpen", en_name: "Sharpen", zh_name: "锐化", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "ADBE Unsharp Mask2", en_name: "Unsharp Mask", zh_name: "钝化蒙版", category: "Blur & Sharpen" },
    BilingualFxEntry { match_name: "ADBE Compound Blur", en_name: "Compound Blur", zh_name: "复合模糊", category: "Blur & Sharpen" },
    // Color Correction
    BilingualFxEntry { match_name: "ADBE CurvesCustom", en_name: "Curves", zh_name: "曲线", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Pro Levels2", en_name: "Levels", zh_name: "色阶", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE HUE SATURATION", en_name: "Hue/Saturation", zh_name: "色相/饱和度", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Tint", en_name: "Tint", zh_name: "色调", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Tritone", en_name: "Tritone", zh_name: "三色调", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Lumetri", en_name: "Lumetri Color", zh_name: "Lumetri 颜色", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Vibrance", en_name: "Vibrance", zh_name: "自然饱和度", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Exposure2", en_name: "Exposure", zh_name: "曝光度", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Brightness & Contrast 2", en_name: "Brightness & Contrast", zh_name: "亮度和对比度", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Black&White", en_name: "Black & White", zh_name: "黑白", category: "Color Correction" },
    BilingualFxEntry { match_name: "CC Toner", en_name: "CC Toner", zh_name: "CC 调色", category: "Color Correction" },
    BilingualFxEntry { match_name: "APC Colorama", en_name: "Colorama", zh_name: "彩色光", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Color Balance (HLS)", en_name: "Color Balance (HLS)", zh_name: "颜色平衡 (HLS)", category: "Color Correction" },
    BilingualFxEntry { match_name: "ADBE Posterize", en_name: "Posterize", zh_name: "色调分离", category: "Color Correction" },
    // Generate
    BilingualFxEntry { match_name: "ADBE Fill", en_name: "Fill", zh_name: "填充", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Ramp", en_name: "Gradient Ramp", zh_name: "渐变", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE 4ColorGradient", en_name: "4-Color Gradient", zh_name: "四色渐变", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Stroke", en_name: "Stroke", zh_name: "描边", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Vegas", en_name: "Vegas", zh_name: "勾画", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Grid", en_name: "Grid", zh_name: "网格", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Checkerboard", en_name: "Checkerboard", zh_name: "棋盘", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Circle", en_name: "Circle", zh_name: "圆形", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Laser", en_name: "Beam", zh_name: "光束", category: "Generate" },
    BilingualFxEntry { match_name: "ADBE Lightning 2", en_name: "Advanced Lightning", zh_name: "高级闪电", category: "Generate" },
    BilingualFxEntry { match_name: "CC Light Sweep", en_name: "CC Light Sweep", zh_name: "CC 扫光", category: "Generate" },
    BilingualFxEntry { match_name: "CC Light Rays", en_name: "CC Light Rays", zh_name: "CC 光线", category: "Generate" },
    BilingualFxEntry { match_name: "CC Light Burst 2.5", en_name: "CC Light Burst 2.5", zh_name: "CC 光芒放射 2.5", category: "Generate" },
    // Stylize
    BilingualFxEntry { match_name: "ADBE Glo2", en_name: "Glow", zh_name: "发光", category: "Stylize" },
    BilingualFxEntry { match_name: "ADBE Drop Shadow", en_name: "Drop Shadow", zh_name: "投影", category: "Perspective" },
    BilingualFxEntry { match_name: "ADBE Bevel Alpha", en_name: "Bevel Alpha", zh_name: "斜面 Alpha", category: "Perspective" },
    BilingualFxEntry { match_name: "ADBE Find Edges", en_name: "Find Edges", zh_name: "查找边缘", category: "Stylize" },
    BilingualFxEntry { match_name: "ADBE Mosaic", en_name: "Mosaic", zh_name: "马赛克", category: "Stylize" },
    BilingualFxEntry { match_name: "ADBE Motion Tile", en_name: "Motion Tile", zh_name: "动态拼贴", category: "Stylize" },
    BilingualFxEntry { match_name: "CC RepeTile", en_name: "CC RepeTile", zh_name: "CC 重复拼贴", category: "Stylize" },
    BilingualFxEntry { match_name: "ADBE Roughen Edges", en_name: "Roughen Edges", zh_name: "毛边", category: "Stylize" },
    BilingualFxEntry { match_name: "ADBE Threshold2", en_name: "Threshold", zh_name: "阈值", category: "Stylize" },
    BilingualFxEntry { match_name: "CC Glass", en_name: "CC Glass", zh_name: "CC 玻璃", category: "Stylize" },
    BilingualFxEntry { match_name: "CC Kaleida", en_name: "CC Kaleida", zh_name: "CC 万花筒", category: "Stylize" },
    // Distort
    BilingualFxEntry { match_name: "ADBE Geometry2", en_name: "Transform", zh_name: "变换", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Turbulent Displace", en_name: "Turbulent Displace", zh_name: "湍流置换", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Displacement Map", en_name: "Displacement Map", zh_name: "置换图", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Wave Warp", en_name: "Wave Warp", zh_name: "波形变形", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Optics Compensation", en_name: "Optics Compensation", zh_name: "光学补偿", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Corner Pin", en_name: "Corner Pin", zh_name: "边角定位", category: "Distort" },
    BilingualFxEntry { match_name: "CC Power Pin", en_name: "CC Power Pin", zh_name: "CC 强力边角定位", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Bezier Warp", en_name: "Bezier Warp", zh_name: "贝塞尔曲线变形", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Mesh Warp", en_name: "Mesh Warp", zh_name: "网格变形", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Liquify", en_name: "Liquify", zh_name: "液化", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Mirror", en_name: "Mirror", zh_name: "镜像", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Offset", en_name: "Offset", zh_name: "偏移", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Polar Coordinates", en_name: "Polar Coordinates", zh_name: "极坐标", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Bulge", en_name: "Bulge", zh_name: "凸出", category: "Distort" },
    BilingualFxEntry { match_name: "ADBE Twirl", en_name: "Twirl", zh_name: "旋转扭曲", category: "Distort" },
    BilingualFxEntry { match_name: "CC Lens", en_name: "CC Lens", zh_name: "CC 镜头", category: "Distort" },
    BilingualFxEntry { match_name: "CC Page Turn", en_name: "CC Page Turn", zh_name: "CC 翻页", category: "Distort" },
    BilingualFxEntry { match_name: "CC Cylinder", en_name: "CC Cylinder", zh_name: "CC 圆柱体", category: "Perspective" },
    BilingualFxEntry { match_name: "CC Sphere", en_name: "CC Sphere", zh_name: "CC 球体", category: "Perspective" },
    // Noise & Grain
    BilingualFxEntry { match_name: "ADBE Fractal Noise", en_name: "Fractal Noise", zh_name: "分形杂色", category: "Noise & Grain" },
    BilingualFxEntry { match_name: "ADBE Noise2", en_name: "Noise", zh_name: "杂色", category: "Noise & Grain" },
    BilingualFxEntry { match_name: "VISINF Grain Implant", en_name: "Add Grain", zh_name: "添加颗粒", category: "Noise & Grain" },
    // Matte & Keying & Channel
    BilingualFxEntry { match_name: "ADBE Simple Choker", en_name: "Simple Choker", zh_name: "简单阻塞工具", category: "Matte" },
    BilingualFxEntry { match_name: "ADBE Matte Choker", en_name: "Matte Choker", zh_name: "遮罩阻塞工具", category: "Matte" },
    BilingualFxEntry { match_name: "ADBE Set Matte3", en_name: "Set Matte", zh_name: "设置遮罩", category: "Channel" },
    BilingualFxEntry { match_name: "ADBE Shift Channels", en_name: "Shift Channels", zh_name: "转换通道", category: "Channel" },
    BilingualFxEntry { match_name: "ADBE Invert", en_name: "Invert", zh_name: "反转", category: "Channel" },
    BilingualFxEntry { match_name: "ADBE Minimax", en_name: "Minimax", zh_name: "最小/最大", category: "Channel" },
    BilingualFxEntry { match_name: "Keylight 906", en_name: "Keylight (1.2)", zh_name: "Keylight (1.2)", category: "Keying" },
    BilingualFxEntry { match_name: "ADBE Linear Color Key2", en_name: "Linear Color Key", zh_name: "线性颜色键", category: "Keying" },
    BilingualFxEntry { match_name: "ADBE Extract", en_name: "Extract", zh_name: "提取", category: "Keying" },
    // Transition & Time
    BilingualFxEntry { match_name: "ADBE Linear Wipe", en_name: "Linear Wipe", zh_name: "线性擦除", category: "Transition" },
    BilingualFxEntry { match_name: "ADBE Radial Wipe", en_name: "Radial Wipe", zh_name: "径向擦除", category: "Transition" },
    BilingualFxEntry { match_name: "ADBE Venetian Blinds", en_name: "Venetian Blinds", zh_name: "百叶窗", category: "Transition" },
    BilingualFxEntry { match_name: "ADBE Block Dissolve", en_name: "Block Dissolve", zh_name: "块溶解", category: "Transition" },
    BilingualFxEntry { match_name: "ADBE Posterize Time", en_name: "Posterize Time", zh_name: "色调分离时间(抽帧)", category: "Time" },
    BilingualFxEntry { match_name: "ADBE Echo", en_name: "Echo", zh_name: "残影", category: "Time" },
    // Expression Controls
    BilingualFxEntry { match_name: "ADBE Slider Control", en_name: "Slider Control", zh_name: "滑块控制", category: "Expression Controls" },
    BilingualFxEntry { match_name: "ADBE Color Control", en_name: "Color Control", zh_name: "颜色控制", category: "Expression Controls" },
    BilingualFxEntry { match_name: "ADBE Checkbox Control", en_name: "Checkbox Control", zh_name: "复选框控制", category: "Expression Controls" },
    BilingualFxEntry { match_name: "ADBE Point Control", en_name: "Point Control", zh_name: "点控制", category: "Expression Controls" },
    BilingualFxEntry { match_name: "ADBE Angle Control", en_name: "Angle Control", zh_name: "角度控制", category: "Expression Controls" },
    BilingualFxEntry { match_name: "ADBE Layer Control", en_name: "Layer Control", zh_name: "图层控制", category: "Expression Controls" },
    BilingualFxEntry { match_name: "ADBE Dropdown Control", en_name: "Dropdown Menu Control", zh_name: "下拉菜单控制", category: "Expression Controls" },
];

pub fn lookup_bilingual(match_name: &str) -> Option<&'static BilingualFxEntry> {
    BUILTIN_AE_EFFECTS
        .iter()
        .find(|entry| entry.match_name == match_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pinyin_conversion_for_chinese_and_mixed_names() {
        let (full, initials) = to_pinyin_keys("高斯模糊");
        assert_eq!(full, "gaosimohu");
        assert_eq!(initials, "gsmh");

        let (full2, initials2) = to_pinyin_keys("CC 径向快速模糊 2");
        assert!(full2.contains("jingxiangkuaisumohu"));
        assert_eq!(initials2, "ccjxksmh2");
    }

    #[test]
    fn test_bilingual_lookup() {
        let g = lookup_bilingual("ADBE Gaussian Blur 2").expect("Gaussian Blur should exist");
        assert_eq!(g.zh_name, "高斯模糊");
        assert_eq!(g.en_name, "Gaussian Blur");
    }
}

