# Qunevo Demo MES brand assets

Reviewed 1 October 2026 against [Qunevo's website](https://www.qunevo.com/)
and its local source checkout.

- `qunevo-logo.svg`: the original SVG geometry and gradient from the website's
  `QunevoLogo` component in `app/icons.tsx`, without the surrounding animation.
  The mark, proportions and colors are unchanged. This is Qunevo's brand asset,
  not a new grant of trademark rights.
- Colors follow `app/globals.css`: primary `hsl(230 82% 53%)`, foreground
  `hsl(222 47% 11%)`, and the logo's `#5433FF` to `#20BDFF` gradient.
- `outfit-latin.woff2` and `bricolage-grotesque-latin.woff2`: unchanged Latin
  webfont files from the website's Next.js build. The same Outfit body font
  and Bricolage Grotesque heading font are served locally, without runtime
  requests to Google Fonts or the main website.
- Font copyright notices and SIL Open Font License 1.1 are retained in
  `OFL-Outfit.txt` and `OFL-BricolageGrotesque.txt`, obtained from the respective
  [Outfit](https://github.com/google/fonts/tree/main/ofl/outfit) and
  [Bricolage Grotesque](https://github.com/google/fonts/tree/main/ofl/bricolagegrotesque)
  Google Fonts directories. Those licenses apply only to the bundled fonts;
  the repository's APEX license is unchanged.
