# wmcr

A small Windows launcher that starts a Minecraft Forge 1.20.1 server and saves its logs before restarting it after a failed exit.

## Build

Install the Rust toolchain, then run:

```powershell
cargo build --release
```

Copy `target\release\wmcr.exe` into the Forge server directory beside `user_jvm_args.txt`, then supply the Java arguments from the Forge `run.bat` command:

```text
wmcr.exe @user_jvm_args.txt @libraries/net/minecraftforge/forge/1.20.1-47.4.22/win_args.txt
```

The launcher forwards every argument after its executable name to `java` in the same order. Add any extra Java arguments at the end, as you would after `%*` in the batch file. Run it from the Forge server directory so relative argument-file paths resolve correctly. For example:

```powershell
cd C:\Servers\ForgeServer
.\wmcr.exe @user_jvm_args.txt @libraries/net/minecraftforge/forge/1.20.1-47.4.22/win_args.txt --nogui
```

When Java exits successfully, the launcher stops. After a failed exit, it copies the crash reports and `logs\latest.log` into a uniquely named directory under `backups`. If Java ran for less than five minutes, the launcher exits with an error instead of retrying. Otherwise, it waits five seconds and starts the server again. The backup directory remains alongside the server files.

Press Ctrl+C to interrupt the Java server and stop the launcher without restarting it.
