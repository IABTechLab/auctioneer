// Only the shutdown regression uses this stub; it never claims browser coverage.
exports.chromium = {
  launch() {
    process.stdout.write('browser-launch-pending\n');
    return new Promise(() => {});
  },
};
