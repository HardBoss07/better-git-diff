#!/usr/bin/env python3
import argparse
import json
import math
import os
import subprocess
import sys
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Optional

os.environ["TOKENIZERS_PARALLELISM"] = "false"
os.environ["HF_HUB_DISABLE_PROGRESS_BARS"] = "1"
os.environ["HF_HUB_DISABLE_TELEMETRY"] = "1"
os.environ["HF_HUB_VERBOSITY"] = "error"
os.environ["TRANSFORMERS_VERBOSITY"] = "error"
os.environ["TRANSFORMERS_NO_ADVISORY_WARNINGS"] = "1"

import tiktoken
from transformers import AutoTokenizer


@dataclass
class BenchmarkRow:
    model: str
    baseline_tokens: int
    optimized_tokens: int


def find_repo_root() -> Path:
    for start in (Path.cwd(), Path(__file__).resolve().parent):
        for candidate in (start, *start.parents):
            if (candidate / "Cargo.toml").is_file():
                return candidate
    return Path.cwd()


def build_binary(repo_root: Path, release: bool) -> Path:
    cmd = ["cargo", "build", "-q"]
    if release:
        cmd.append("--release")

    res = subprocess.run(
        cmd,
        cwd=repo_root,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if res.returncode != 0:
        sys.exit(f"Cargo build failed:\n{res.stderr.strip()}")

    profile = "release" if release else "debug"
    suffix = ".exe" if os.name == "nt" else ""
    bin_path = repo_root / "target" / profile / f"better-git-diff{suffix}"
    if not bin_path.is_file():
        sys.exit(f"Built binary not found at {bin_path}")
    return bin_path


def run_cmd(cmd: list[str], cwd: Path) -> str:
    res = subprocess.run(
        cmd,
        cwd=cwd,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if res.returncode != 0:
        sys.exit(f"Command failed ({' '.join(cmd)}):\n{res.stderr.strip()}")
    return res.stdout


def format_delta(baseline: int, optimized: int) -> str:
    if baseline == 0:
        return "0.0% (0)"
    saved = baseline - optimized
    pct = (saved / baseline) * 100.0
    if saved >= 0:
        return f"-{pct:.1f}% (-{saved:,})"
    return f"+{abs(pct):.1f}% (+{abs(saved):,})"


def load_hf_tokenizer(candidates: list[str]):
    for repo_id in candidates:
        for local_only in (True, False):
            try:
                return AutoTokenizer.from_pretrained(
                    repo_id,
                    trust_remote_code=False,
                    local_files_only=local_only,
                )
            except Exception:
                continue
    return None


def count_anthropic_api(text: str, api_key: str) -> Optional[int]:
    req = urllib.request.Request(
        "https://api.anthropic.com/v1/messages/count_tokens",
        data=json.dumps({
            "model": "claude-sonnet-4-6",
            "messages": [{"role": "user", "content": text}],
        }).encode("utf-8"),
        headers={
            "x-api-key": api_key,
            "anthropic-version": "2023-06-01",
            "content-type": "application/json",
        },
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8")).get("input_tokens")
    except Exception:
        return None


def count_gemini_api(text: str, api_key: str) -> Optional[int]:
    url = (
        "https://generativelanguage.googleapis.com/v1beta/models/"
        f"gemini-2.5-flash:countTokens?key={api_key}"
    )
    req = urllib.request.Request(
        url,
        data=json.dumps({"contents": [{"parts": [{"text": text}]}]}).encode("utf-8"),
        headers={"content-type": "application/json"},
        method="POST",
    )
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            return json.loads(resp.read().decode("utf-8")).get("totalTokens")
    except Exception:
        return None


def collect_benchmarks(base_text: str, opt_text: str) -> list[BenchmarkRow]:
    rows: list[BenchmarkRow] = []

    o200k = tiktoken.get_encoding("o200k_base")
    rows.append(BenchmarkRow(
        model="GPT-6.1 Sol / Astra / Luna (`o200k_base`)",
        baseline_tokens=len(o200k.encode_ordinary(base_text)),
        optimized_tokens=len(o200k.encode_ordinary(opt_text)),
    ))

    cl100k = tiktoken.get_encoding("cl100k_base")
    rows.append(BenchmarkRow(
        model="GPT-4 / 3.5 (`cl100k_base`)",
        baseline_tokens=len(cl100k.encode_ordinary(base_text)),
        optimized_tokens=len(cl100k.encode_ordinary(opt_text)),
    ))

    hf_specs = [
        (
            "DeepSeek V4.1 Flash / V4 Pro",
            ["deepseek-ai/DeepSeek-V4.1-Flash", "deepseek-ai/DeepSeek-V3"],
        ),
        (
            "Qwen 3 / 3.8",
            ["Qwen/Qwen3-0.6B", "Qwen/Qwen2.5-7B-Instruct"],
        ),
        (
            "Mistral Large 4",
            ["mistralai/Mistral-Large-4-1T-A52B", "mistralai/Mistral-7B-Instruct-v0.3"],
        ),
    ]

    for label, candidates in hf_specs:
        tok = load_hf_tokenizer(candidates)
        if tok is not None:
            rows.append(BenchmarkRow(
                model=label,
                baseline_tokens=len(tok.encode(base_text, add_special_tokens=False)),
                optimized_tokens=len(tok.encode(opt_text, add_special_tokens=False)),
            ))

    gemini_key = os.environ.get("GEMINI_API_KEY", "").strip()
    gemini_exact = None
    if gemini_key:
        b_g = count_gemini_api(base_text, gemini_key)
        m_g = count_gemini_api(opt_text, gemini_key)
        if b_g is not None and m_g is not None:
            gemini_exact = (b_g, m_g)

    if gemini_exact is not None:
        rows.append(BenchmarkRow(
            model="Gemini 3.8 Flash / 3.1 Pro",
            baseline_tokens=gemini_exact[0],
            optimized_tokens=gemini_exact[1],
        ))
    else:
        gemma_tok = load_hf_tokenizer(["unsloth/gemma-3-1b-it"])
        if gemma_tok is not None:
            rows.append(BenchmarkRow(
                model="Gemini 3.8 Flash / 3.1 Pro *(proxy)*",
                baseline_tokens=len(gemma_tok.encode(base_text, add_special_tokens=False)),
                optimized_tokens=len(gemma_tok.encode(opt_text, add_special_tokens=False)),
            ))

    anthropic_key = os.environ.get("ANTHROPIC_API_KEY", "").strip()
    claude_exact = None
    if anthropic_key:
        b_c = count_anthropic_api(base_text, anthropic_key)
        m_c = count_anthropic_api(opt_text, anthropic_key)
        if b_c is not None and m_c is not None:
            claude_exact = (b_c, m_c)

    if claude_exact is not None:
        rows.append(BenchmarkRow(
            model="Claude Fable 5.1 / Opus 5.5 / Sonnet 5.5",
            baseline_tokens=claude_exact[0],
            optimized_tokens=claude_exact[1],
        ))
    else:
        rows.append(BenchmarkRow(
            model="Claude Fable 5.1 / Opus 5.5 / Sonnet 5.5 *(proxy)*",
            baseline_tokens=math.ceil(len(base_text) / 3.5),
            optimized_tokens=math.ceil(len(opt_text) / 3.5),
        ))

    return rows


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Build better-git-diff and benchmark its token usage against regular git diff"
    )
    parser.add_argument(
        "--preset",
        choices=["ai", "default", "human"],
        default="ai",
        help="better-git-diff preset to benchmark (default: ai)",
    )
    parser.add_argument(
        "--release",
        action="store_true",
        help="Build with --release before benchmarking",
    )
    parser.add_argument(
        "--files",
        nargs=2,
        metavar=("GIT_DIFF", "BGD_DIFF"),
        help="Compare two existing diff files instead of building and running live",
    )
    parser.add_argument(
        "git_args",
        nargs="*",
        help="Shared diff arguments forwarded to both git diff and better-git-diff",
    )
    args = parser.parse_args()

    if args.files:
        p1, p2 = Path(args.files[0]), Path(args.files[1])
        base_desc, opt_desc = p1.name, p2.name
        base_col, opt_col = p1.name, p2.name
        base_text = p1.read_text(encoding="utf-8", errors="replace")
        opt_text = p2.read_text(encoding="utf-8", errors="replace")
    else:
        repo_root = find_repo_root()
        bin_path = build_binary(repo_root, release=args.release)

        preset_flags = {
            "ai": ["--ai"],
            "default": [],
            "human": ["--human"],
        }[args.preset]

        bgd_flags = [*preset_flags, "--all-files"]
        base_desc = " ".join(["git diff", *args.git_args])
        opt_desc = " ".join(["better-git-diff", *bgd_flags, *args.git_args])
        base_col, opt_col = "git diff", "better-git-diff"

        base_text = run_cmd(
            ["git", "--no-pager", "diff", "--no-color", *args.git_args],
            cwd=repo_root,
        )
        opt_text = run_cmd(
            [str(bin_path), *bgd_flags, "--no-color", *args.git_args],
            cwd=repo_root,
        )

    if not base_text.strip() and not opt_text.strip():
        print("No diff output to benchmark (working tree clean). Pass --staged or a range like HEAD~1..HEAD.")
        return

    base_lines, base_chars = len(base_text.splitlines()), len(base_text)
    opt_lines, opt_chars = len(opt_text.splitlines()), len(opt_text)

    print("**Benchmark Config & Size**")
    print(f"- **Baseline:** `{base_desc}` ({base_lines:,} lines | {base_chars:,} chars)")
    print(f"- **Target:** `{opt_desc}` ({opt_lines:,} lines | {opt_chars:,} chars)")
    print(f"- **Size Reduction:** {format_delta(base_lines, opt_lines)} lines | {format_delta(base_chars, opt_chars)} chars\n")

    rows = collect_benchmarks(base_text, opt_text)

    print(f"| Model | `{base_col}` | `{opt_col}` | Improvement |")
    print("| :--- | ---: | ---: | ---: |")
    for r in rows:
        print(
            f"| {r.model} | {r.baseline_tokens:,} | {r.optimized_tokens:,} | "
            f"**{format_delta(r.baseline_tokens, r.optimized_tokens)}** |"
        )


if __name__ == "__main__":
    main()