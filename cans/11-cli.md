- CLI
  - Inspection commands
    - logs tails guest output with cursor based replay
      - Cursors resume a tail from an earlier point
      - History survives sandbox restarts
      - Underlying guarantee lives in 04-terminal-plane.md#Stream-replay;
        the CLI is a thin consumer with no additional semantics