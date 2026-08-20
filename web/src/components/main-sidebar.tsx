import * as React from "react"
import {
  CalendarClock,
  CheckCheck,
  FileText,
  FolderKanban,
  Settings2,
  Sparkles,
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
  { key: "ready", label: "Ready", icon: <CheckCheck /> },
  { key: "scheduled", label: "Scheduled", icon: <CalendarClock /> },
]

type MainSidebarProps = React.ComponentProps<typeof Sidebar> & {
  activePage: AppPage
  onPageChange: (page: AppPage) => void
}

export function MainSidebar({
  activePage,
  onPageChange,
  ...props
}: MainSidebarProps) {
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
