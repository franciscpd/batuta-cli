# compozy-testkit

Development-only harness for contract and end-to-end tests. It launches the
real CompozyOS daemon in a unique temporary `COMPOZY_HOME`, records requests,
and stops the complete daemon process group during teardown.

Contract tests must run from a disposable detached worktree that does not
contain `.compozy/`:

```sh
COMPOZY_TEST_DAEMON_BIN=$(which compozy) cargo test -p compozy-client --test contract
```

When neither `COMPOZY_TEST_DAEMON_BIN` nor `compozy` on `PATH` is available,
the contract binary prints `skipped: set COMPOZY_TEST_DAEMON_BIN` and passes.
