# Wiki publication

The [public wiki](https://github.com/qunevo/apex/wiki) is generated from reviewed repository documentation. Edit the source files through a pull request. No AI service, model API, personal access token or third-party publishing action is used by the workflow.

## Source and publication

`dev/scripts/wiki-pages.json` is the explicit public-page allowlist and navigation order. Page labels come from each document's first level-one heading. `app/docs/README.md` becomes `Home`; `_Sidebar.md` and `_Footer.md` are generated. New files are not automatically published. Review a new page's contents before adding it to the allowlist.

`python dev/scripts/build_wiki.py` validates the allowlist and local Markdown link targets without writing files. The exporter supports inline links and images, reference link definitions, fragments and link titles. Fenced and inline code examples are preserved. It does not fetch external URLs or rewrite HTML links; use Markdown links in published documents. Links between exported documents lead to wiki pages. Links to source, examples, schemas and the canonical license point to the exact source commit on GitHub.

For a local preview:

```bash
python -m unittest discover -s dev/tests -p 'test_wiki.py' -v
python dev/scripts/build_wiki.py --output build/wiki
```

Generated pages are not committed to the main repository. The wiki keeps ordinary Git history. The exporter records managed filenames, removes obsolete managed pages and preserves unrelated pages. It refuses symlinks and refuses to overwrite pages without its generated marker. On first adoption, review existing pages before marking them as generated; do not silently discard handwritten content.

## GitHub setup

Enable Wikis and keep editing restricted to collaborators with push access. Initialize the wiki by creating its first `Home` page in GitHub; GitHub requires this before its separate Git repository can be cloned. The initial generated Home page must start with the exporter's marker.

The `Publish` workflow in `.github/workflows/wiki.yml` runs only for successful main push CI in `qunevo/apex`, or a manual retry on main that independently verifies CI. Pull-request validation lives in the read-only CI workflow. Publication uses the job-scoped Actions token with `contents: write` for source releases and the wiki push, plus `actions: read` to verify the successful run. No reusable credential is stored. Authentication or publication failure fails visibly.

Publication runs are serialized with GitHub's `queue: max` (up to 100 pending runs) and check out the exact successful CI revision. This avoids the default concurrency behavior of replacing an older pending publication. The publisher verifies that it is still current main before updating the wiki; an older queued run skips the wiki rather than rolling documentation back. A second current-main check stops publication if main advanced meanwhile. There are no force pushes. Re-running an unchanged revision creates no wiki commit. The agent can retry a failed workflow after fixing its cause; using GitHub's Actions page is optional. See [GitHub concurrency](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/control-workflow-concurrency).

`actionlint` v1.7.12 predates the documented `concurrency.queue` key. When using that version locally, narrowly ignore only `unexpected key "queue" for "concurrency" section`; keep all other workflow diagnostics enabled. Validate this key against the GitHub reference above until the linter supports it.

The general CI's `Source publication` job also runs the wiki tests and link validation. Its result is included in the required `Required checks` aggregate, so invalid documentation blocks merging. Wiki edits do not use the main repository's pull-request review flow; maintainers should submit documentation changes to the main repository instead. Direct edits to generated wiki pages are replaced during publication. Review and merge requirements for `main` remain in force.

The website's Documentation link points to the wiki. The maintained documentation is English; the website's navigation labels are available in German and English.
