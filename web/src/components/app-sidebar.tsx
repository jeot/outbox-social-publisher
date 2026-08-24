import * as React from "react"
import {
  CalendarClock,
  CheckCheck,
  FileText,
  FolderKanban,
  Settings2,
  Sparkles,
  SunMoon,
} from "lucide-react"
import { NavUser } from "@/components/nav-user"
import { TeamSwitcher } from "@/components/team-switcher"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"

export type AppPage = "catalog" | "ready" | "scheduled"

const teams = [
  { name: "Publo", logo: <Sparkles className="size-4" />, plan: "Local mode" },
]

const pages: { key: AppPage; label: string; icon: React.ReactNode }[] = [
  { key: "catalog", label: "Catalog", icon: <FolderKanban /> },
  { key: "ready", label: "Decision Queue", icon: <CheckCheck /> },
  { key: "scheduled", label: "Scheduled", icon: <CalendarClock /> },
]

type AppSidebarProps = React.ComponentProps<typeof Sidebar> & {
  activePage: AppPage
  onPageChange: (page: AppPage) => void
}

export function AppSidebar({
  activePage,
  onPageChange,
  ...props
}: AppSidebarProps) {
  const [theme, setTheme] = React.useState<"light" | "dark">("light")

  React.useEffect(() => {
    const saved = window.localStorage.getItem("publo.theme")
    if (saved === "light" || saved === "dark") {
      setTheme(saved)
      document.documentElement.classList.toggle("dark", saved === "dark")
      return
    }

    const systemDark = window.matchMedia?.("(prefers-color-scheme: dark)").matches
    const initial: "light" | "dark" = systemDark ? "dark" : "light"
    setTheme(initial)
    document.documentElement.classList.toggle("dark", initial === "dark")
  }, [])

  const toggleTheme = React.useCallback(() => {
    const next: "light" | "dark" = theme === "dark" ? "light" : "dark"
    setTheme(next)
    document.documentElement.classList.toggle("dark", next === "dark")
    window.localStorage.setItem("publo.theme", next)
  }, [theme])

  return (
    <Sidebar collapsible="icon" variant="inset" {...props}>
      <SidebarHeader>
        <TeamSwitcher teams={teams} />
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup>
          <SidebarGroupLabel>Pipeline</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              {pages.map((page) => (
                <SidebarMenuItem key={page.key}>
                  <SidebarMenuButton
                    isActive={activePage === page.key}
                    tooltip={page.label}
                    render={
                      <button
                        type="button"
                        onClick={() => onPageChange(page.key)}
                      />
                    }
                  >
                    {page.icon}
                    <span>{page.label}</span>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              ))}
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>

        <SidebarGroup className="mt-auto">
          <SidebarGroupLabel>Workspace</SidebarGroupLabel>
          <SidebarGroupContent>
            <SidebarMenu>
              <SidebarMenuItem>
                <SidebarMenuButton tooltip="Files">
                  <FileText />
                  <span>Files</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton tooltip="Settings">
                  <Settings2 />
                  <span>Settings</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
              <SidebarMenuItem>
                <SidebarMenuButton
                  tooltip={theme === "dark" ? "Switch to light" : "Switch to dark"}
                  render={<button type="button" onClick={toggleTheme} />}
                >
                  <SunMoon />
                  <span>{theme === "dark" ? "Light mode" : "Dark mode"}</span>
                </SidebarMenuButton>
              </SidebarMenuItem>
            </SidebarMenu>
          </SidebarGroupContent>
        </SidebarGroup>
      </SidebarContent>
      <SidebarFooter>
        <NavUser
          user={{
            name: "Local User",
            email: "you@localhost",
            avatar: "/favicon.svg",
          }}
        />
      </SidebarFooter>
    </Sidebar>
  )
}
