# Fonts

Not committed (size + licence hygiene). Run `scripts/fetch-fonts.sh` once; it drops static Inter TTFs here.

| File | Weight | CSS `font-weight` |
|---|---|---|
| Inter-Regular.ttf | 400 | `font-normal` |
| Inter-Medium.ttf | 500 | `font-medium` |
| Inter-SemiBold.ttf | 600 | `font-semibold` |
| Inter-Bold.ttf | 700 | `font-bold` |
| Inter-ExtraBold.ttf | 800 | `font-extrabold` |

htmltopdf never uses system fonts (`system_fonts(false)`), so every weight a fixture uses must be bundled,
otherwise the engine synthesises or falls back and the pixel diff against Chrome is meaningless.
Inter is licensed under the SIL Open Font License 1.1.
