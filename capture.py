"""Renode capture sequence for Key Ceremony (not yet run).

Run from xous-dev-toolkit:
    python3 scripts/renode_capture.py --app keygen --script ../precursor-keygen/capture.py

Contract: run(ctl, outdir) -> number of failed steps. The app is already launched.
"""
import os


def run(ctl, outdir) -> int:
    failures = 0

    def shot(name):
        nonlocal failures
        if not ctl.try_screenshot(os.path.join(outdir, name)):
            failures += 1

    shot("01_types.png")
    ctl.timed_key("Down", after=1.0)
    ctl.timed_key("Down", after=1.0)
    ctl.inject_line("")            # Enter: configure Passphrase
    ctl.sleep(1.0)
    ctl.timed_key("Right", after=1.0)
    shot("02_configure.png")
    ctl.inject_line("")            # Enter: generate
    ctl.sleep(1.0)
    shot("03_result.png")
    ctl.inject_key("s")            # save -> label screen
    ctl.inject_string("demo")
    shot("04_label.png")
    ctl.inject_line("")            # Enter: save
    ctl.sleep(1.5)
    shot("05_saved_status.png")
    for _ in range(2):             # back to the type list
        ctl.timed_key("F4", after=1.0)
    ctl.inject_key("s")            # saved list
    ctl.sleep(1.0)
    shot("06_saved_list.png")
    ctl.inject_line("")            # view full value
    ctl.sleep(1.0)
    shot("07_view.png")
    ctl.timed_key("BackSpace", after=1.0)
    ctl.timed_key("BackSpace", after=1.0)
    shot("08_back_to_types.png")
    return failures
