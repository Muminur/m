/** Shared state and callbacks passed from IntegrationWizard to each tab */
export interface TabSharedProps {
  transcriptId?: string;
  saving: boolean;
  testing: boolean;
  isLoading: boolean;
  setStatus: (status: { type: "success" | "error"; message: string } | null) => void;
  clearStatus: () => void;
  setSaving: (saving: boolean) => void;
  setTesting: (testing: boolean) => void;
}
