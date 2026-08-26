{ pkgs, extensionId ? "p1-22-spike@deepwork-focus-bot.invalid" }:

let
  host = pkgs.writeShellScript "deepwork-focus-p1-22-native-host" ''
    exec ${pkgs.nodejs}/bin/node ${./../native/host.js} "$@"
  '';
in
pkgs.writeTextDir "lib/mozilla/native-messaging-hosts/org.deepwork_focus.p1_22_spike.json" (builtins.toJSON {
  name = "org.deepwork_focus.p1_22_spike";
  description = "Deepwork Focus native messaging host";
  path = host;
  type = "stdio";
  allowed_extensions = [ extensionId ];
})
