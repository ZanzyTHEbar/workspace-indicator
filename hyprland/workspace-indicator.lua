-- Workspace indicator OSD — Hyprland 0.56+ & Omarchy Quattro Lua integration
hl.layer_rule({
  name         = "workspace-indicator",
  match        = { namespace = "workspace-indicator" },
  blur         = true,
  ignore_alpha = 0.3,
})

local peek_cmd = "workspace-indicator --peek"
if type(o) == "table" and type(o.bind) == "function" then
  o.bind("SUPER + grave", "Show workspace indicator", peek_cmd)
else
  hl.bind("SUPER + grave", hl.dsp.exec_cmd(peek_cmd), { description = "Show workspace indicator" })
end

hl.on("hyprland.start", function()
  hl.exec_cmd("command -v systemctl >/dev/null 2>&1 && systemctl --user start workspace-indicator.service")
end)
