//! Task Scheduler XML for Apex Tune's own logon tasks, and the small bit of
//! parsing needed to check an existing task's command.

/// Escapes text for an XML element body.
pub fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

pub struct LogonTask<'a> {
    /// `DOMAIN\user` the task runs as, at that user's logon.
    pub account: &'a str,
    pub command: &'a str,
    pub arguments: &'a str,
    pub description: &'a str,
    /// Run elevated ("Run with highest privileges").
    pub highest: bool,
}

impl LogonTask<'_> {
    pub fn to_xml(&self) -> String {
        let run_level = if self.highest {
            "HighestAvailable"
        } else {
            "LeastPrivilege"
        };
        format!(
            r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>Apex Tune</Author>
    <Description>{description}</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{account}</UserId>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{account}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>{run_level}</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <Arguments>{arguments}</Arguments>
    </Exec>
  </Actions>
</Task>
"#,
            description = escape(self.description),
            account = escape(self.account),
            run_level = run_level,
            command = escape(self.command),
            arguments = escape(self.arguments),
        )
    }
}

fn element<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{tag}>"))? + start;
    Some(&xml[start..end])
}

/// The first Exec action's command and arguments.
pub fn exec_action(xml: &str) -> Option<(String, String)> {
    let exec = element(xml, "Exec")?;
    let command = unescape(element(exec, "Command")?.trim().trim_matches('"'));
    let arguments = element(exec, "Arguments")
        .map(|a| unescape(a.trim()))
        .unwrap_or_default();
    Some((command, arguments))
}

/// The principal's user, used when re-registering a captured task.
pub fn principal_user(xml: &str) -> Option<String> {
    element(element(xml, "Principals")?, "UserId").map(unescape)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_command_and_escapes_values() {
        let task = LogonTask {
            account: r"PC\Kai & Co",
            command: r"C:\Program Files\Apex Tune\apex-timer.exe",
            arguments: "--resolution 5000",
            description: "Holds <timer> resolution",
            highest: false,
        };
        let xml = task.to_xml();
        assert!(xml.contains("<LogonType>InteractiveToken</LogonType>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(xml.contains("Kai &amp; Co"));
        assert_eq!(
            exec_action(&xml),
            Some((task.command.to_owned(), task.arguments.to_owned()))
        );
        assert_eq!(principal_user(&xml).as_deref(), Some(r"PC\Kai & Co"));
    }

    #[test]
    fn reads_windows_formatted_tasks() {
        let xml = r#"<Task><Actions Context="Author"><Exec><Command>"C:\x\y.exe"</Command></Exec></Actions></Task>"#;
        assert_eq!(
            exec_action(xml),
            Some((r"C:\x\y.exe".into(), String::new()))
        );
        assert_eq!(exec_action("<Task/>"), None);
    }
}
