import {
  AboutSection,
  AgentsSection,
  AppearanceSection,
  GeneralSection,
  ModelsSection,
  NotificationsSection,
  WithSettings,
} from "@/features/settings";

/*
 * The app-settings sections of the Settings dialog. The dialog frame and its section
 * registry live in the app layout (`pages/app/layout/settings/`), which composes these
 * with the Design Library and Role Handbook widgets.
 */

export function GeneralSettings() {
  return <WithSettings>{(settings) => <GeneralSection settings={settings} />}</WithSettings>;
}

export function AppearanceSettings() {
  return <WithSettings>{(settings) => <AppearanceSection settings={settings} />}</WithSettings>;
}

export function ModelsSettings() {
  return <WithSettings>{(settings) => <ModelsSection settings={settings} />}</WithSettings>;
}

export function AgentsSettings() {
  return <WithSettings>{(settings) => <AgentsSection settings={settings} />}</WithSettings>;
}

export function NotificationsSettings() {
  return <WithSettings>{(settings) => <NotificationsSection settings={settings} />}</WithSettings>;
}

export function AboutSettings() {
  return <AboutSection />;
}
