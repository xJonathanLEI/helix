#!/bin/bash

set -e
set -x

# This script assumes CWD is the Helix repo. Run it at the repo root.

# The list of PRs to pick here:
INTERESTING_PRS=(
  # Fix old values shown in `select_register`
  5242
  # Make search commands respect register selection
  5244
  # Support going to specific positions in file
  5260
  # Make mouse click extend selection in select mode
  5436
  # Only render the auto-complete menu if it intersects with signature help
  # 5523 (conflicts)
  # Changed file picker
  5645
  # Inline Diagnostics
  6417
)

# Makes the latest PR head available at a local branch
function fetch_pr() {
  PR="$1"

  git branch -D pr/$PR || true
  git fetch origin refs/pull/$PR/head:pr/$PR
}

# Squashs the PR into the local `batteries` branch
function add_pr() {
  PR="$1"

  git branch -D temp || true
  git checkout -b temp

  git reset --hard pr/$PR
  git rebase batteries

  git reset batteries
  git add .

  # We don't add the "#" before the PR number to avoid spamming the PR thread
  git commit -m "PR $PR"

  git checkout batteries
  git reset --hard temp

  git branch -D temp
}

git fetch origin
git checkout master

git branch -D batteries || true
git checkout -b batteries
git reset --hard origin/master

# Updates the PRs first so that we still have latest heads even if rebase fails
for PR in ${INTERESTING_PRS[@]}; do
  fetch_pr $PR
done

# Actual rebasing and squashing
for PR in ${INTERESTING_PRS[@]}; do
  add_pr $PR
done

# Additional local stuff here
# git cherry-pick ..dev/abc
# git cherry-pick ..dev/def

# Install the branch with this command
# cargo install --locked --path helix-term
