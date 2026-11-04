import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Check, ChevronLeft, ChevronRight, LoaderCircle, Sparkles, X } from 'lucide-react';

interface SetupWizardProps {
  isOpen: boolean;
  onClose: () => void;
  onComplete: () => void;
}

interface UserProfile {
  full_name: string;
  email: string;
  phone: string | null;
  location: string | null;
  linkedin: string | null;
  github: string | null;
  portfolio: string | null;
  target_roles: string[];
  years_experience: number | null;
  current_title: string | null;
}

interface ClaimSuggestion {
  statement: string;
  evidence_source: string;
  confirmed_wording: string;
  scope: string;
  category: string;
}

interface OllamaModel {
  name: string;
  size: number;
  modified_at: string;
}

interface SetupWizardData {
  profile: UserProfile | null;
  resume_content: string;
  selected_ollama_models: string[];
  presentation_rule: {
    date_policy: string;
    tenure_phrasing: string;
    employer_neutral_metrics: boolean;
  } | null;
}

interface PresentationRule {
  section: string;
  date_policy: string;
  tenure_phrasing: string;
  employer_neutral_metrics: boolean;
}

const emptyProfile: UserProfile = {
  full_name: '',
  email: '',
  phone: null,
  location: null,
  linkedin: null,
  github: null,
  portfolio: null,
  target_roles: [],
  years_experience: null,
  current_title: null,
};

const stages = [
  { title: 'Welcome to CareerOS', detail: 'Let’s get your workspace ready so you can explore with confidence.' },
  { title: 'Your profile', detail: 'A few details personalize your workspace.' },
  { title: 'Master resume', detail: 'Add the resume you want CareerOS to work from.' },
  { title: 'Review claims', detail: 'Choose only statements you can personally verify.' },
  { title: 'Local models', detail: 'Connect an Ollama model now or set one up later.' },
  { title: 'Presentation preferences', detail: 'Choose how resume details should be presented.' },
];

export default function SetupWizard({ isOpen, onClose, onComplete }: SetupWizardProps) {
  const [stage, setStage] = useState(0);
  const [profile, setProfile] = useState<UserProfile>(emptyProfile);
  const [rolesInput, setRolesInput] = useState('');
  const [resume, setResume] = useState('');
  const [claims, setClaims] = useState<ClaimSuggestion[]>([]);
  const [selectedClaims, setSelectedClaims] = useState<number[]>([]);
  const [models, setModels] = useState<OllamaModel[]>([]);
  const [selectedModels, setSelectedModels] = useState<string[]>([]);
  const [newModel, setNewModel] = useState('');
  const [datePolicy, setDatePolicy] = useState('month_year');
  const [tenurePhrasing, setTenurePhrasing] = useState('exact');
  const [employerNeutralMetrics, setEmployerNeutralMetrics] = useState(false);
  const [loading, setLoading] = useState(false);
  const [loadingData, setLoadingData] = useState(true);
  const [error, setError] = useState('');
  const loadingRef = useRef(false);

  useEffect(() => {
    loadingRef.current = loading;
  }, [loading]);

  useEffect(() => {
    if (!isOpen) return;

    let active = true;
    setLoadingData(true);
    setError('');
    Promise.all([
      invoke<SetupWizardData>('get_setup_wizard_data'),
      invoke<OllamaModel[]>('get_available_ollama_models'),
    ]).then(([data, availableModels]) => {
      if (!active) return;
      const savedProfile = data.profile ?? emptyProfile;
      setProfile(savedProfile);
      setRolesInput(savedProfile.target_roles.join(', '));
      setResume(data.resume_content);
      setSelectedModels(data.selected_ollama_models);
      setModels(availableModels);
      if (data.presentation_rule) {
        setDatePolicy(data.presentation_rule.date_policy);
        setTenurePhrasing(data.presentation_rule.tenure_phrasing);
        setEmployerNeutralMetrics(data.presentation_rule.employer_neutral_metrics);
      }
    }).catch((err: unknown) => {
      if (active) setError(`Could not load setup data: ${String(err)}`);
    }).finally(() => {
      if (active) setLoadingData(false);
    });

    return () => {
      active = false;
    };
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !loadingRef.current) onClose();
    };
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  if (!isOpen) return null;

  const saveProfile = async () => {
    const hasProfile = Object.values(profile).some((value) =>
      Array.isArray(value) ? value.length > 0 : value !== null && String(value).trim().length > 0,
    ) || rolesInput.trim().length > 0;
    if (!hasProfile) return;

    const normalizedProfile: UserProfile = {
      ...profile,
      target_roles: rolesInput.split(',').map((role) => role.trim()).filter(Boolean),
    };
    await invoke('save_user_profile', { profile: normalizedProfile });
    setProfile(normalizedProfile);
  };

  const continueFromProfile = async () => {
    setLoading(true);
    setError('');
    try {
      await saveProfile();
      setStage(2);
    } catch (err) {
      setError(`Could not save your profile: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const continueFromResume = async () => {
    setLoading(true);
    setError('');
    try {
      if (resume.trim()) {
        await invoke('save_resume_content', { content: resume });
        const suggestions = await invoke<ClaimSuggestion[]>('extract_claims_from_resume', { resumeText: resume });
        setClaims(suggestions);
        setSelectedClaims([]);
      } else {
        setClaims([]);
        setSelectedClaims([]);
      }
      setStage(3);
    } catch (err) {
      setError(`Could not save or review your resume: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const continueFromClaims = async () => {
    setLoading(true);
    setError('');
    try {
      const confirmedClaims = selectedClaims.map((index) => ({
        ...claims[index],
        status: 'confirmed',
      }));
      await invoke('save_claims', { claims: confirmedClaims });
      setStage(4);
    } catch (err) {
      setError(`Could not save your verified claims: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const pullModel = async () => {
    const modelName = newModel.trim();
    if (!modelName) return;
    setLoading(true);
    setError('');
    try {
      const succeeded = await invoke<boolean>('pull_ollama_model', { modelName });
      if (!succeeded) {
        throw new Error('Ollama did not complete the model download. Check the model name and try again.');
      }
      const availableModels = await invoke<OllamaModel[]>('get_available_ollama_models');
      setModels(availableModels);
      setSelectedModels((current) => current.includes(modelName) ? current : [...current, modelName]);
      setNewModel('');
    } catch (err) {
      setError(`Could not download the model: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const finishSetup = async () => {
    setLoading(true);
    setError('');
    try {
      if (selectedModels.length > 0) {
        await invoke('save_ollama_config', {
          fast_model: selectedModels[0],
          deep_model: selectedModels[1] ?? selectedModels[0],
        });
      }
      const presentationRule: PresentationRule = {
        section: 'resume',
        date_policy: datePolicy,
        tenure_phrasing: tenurePhrasing,
        employer_neutral_metrics: employerNeutralMetrics,
      };
      await invoke('save_presentation_rules', { rules: [presentationRule] });
      await invoke('complete_setup_wizard', { ollamaModels: selectedModels });
      setStage(6);
    } catch (err) {
      setError(`Could not finish setup: ${String(err)}`);
    } finally {
      setLoading(false);
    }
  };

  const toggleClaim = (index: number) => {
    setSelectedClaims((current) =>
      current.includes(index) ? current.filter((value) => value !== index) : [...current, index],
    );
  };

  const toggleModel = (name: string) => {
    setSelectedModels((current) =>
      current.includes(name) ? current.filter((model) => model !== name) : [...current, name],
    );
  };

  const updateProfile = (key: keyof UserProfile, value: string) => {
    setProfile((current) => ({ ...current, [key]: value || null }));
  };

  const stepAction = [
    () => setStage(1),
    continueFromProfile,
    continueFromResume,
    continueFromClaims,
    () => setStage(5),
    finishSetup,
    onComplete,
  ][stage];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4" role="presentation">
      <section
        role="dialog"
        aria-modal="true"
        aria-labelledby="setup-title"
        className="flex max-h-[92vh] w-full max-w-3xl flex-col overflow-hidden rounded-xl border border-aether-border bg-aether-surface shadow-2xl"
      >
        <header className="flex items-start justify-between border-b border-aether-border px-6 py-5">
          <div>
            <div className="mb-2 flex items-center gap-2 text-sm text-aether-primary">
              <Sparkles size={16} aria-hidden="true" />
              <span>CareerOS setup</span>
            </div>
            <h2 id="setup-title" className="text-xl font-bold text-aether-text">
              {stages[stage]?.title ?? 'Your workspace is ready'}
            </h2>
            <p className="mt-1 text-sm text-aether-text-muted">
              {stages[stage]?.detail ?? 'You can return to the setup guide from the top bar at any time.'}
            </p>
          </div>
          <button
            type="button"
            onClick={onClose}
            disabled={loading}
            aria-label="Close setup guide"
            className="rounded p-2 text-aether-text-muted transition hover:bg-aether-bg hover:text-aether-text focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary disabled:opacity-50"
          >
            <X size={18} />
          </button>
        </header>

        <div className="border-b border-aether-border px-6 py-4">
          <div className="mb-2 flex justify-between text-xs text-aether-text-muted">
            <span>{stage < stages.length ? `Step ${stage + 1} of ${stages.length}` : 'Setup complete'}</span>
            <span>{Math.round((Math.min(stage + 1, stages.length) / stages.length) * 100)}% complete</span>
          </div>
          <div className="flex gap-2" aria-label={stage < stages.length ? `Step ${stage + 1} of ${stages.length}` : 'Setup complete'}>
            {stages.map((item, index) => (
              <div
                key={item.title}
                className={`h-1.5 flex-1 rounded-full ${index <= stage ? 'bg-aether-primary' : 'bg-aether-border'}`}
              />
            ))}
          </div>
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto px-6 py-5">
          {loadingData ? (
            <div className="flex min-h-48 items-center justify-center gap-3 text-aether-text-muted" role="status">
              <LoaderCircle size={18} className="animate-spin" />
              Loading your setup…
            </div>
          ) : (
            <>
              {stage === 0 && (
                <div className="space-y-5">
                  <p className="max-w-2xl text-sm leading-6 text-aether-text-muted">
                    This short guide helps you personalize CareerOS, review resume claims, and choose optional local AI settings. Your workspace stays on this device, and you can skip any part or come back later.
                  </p>
                  <div className="rounded-lg border border-aether-border bg-aether-bg p-4">
                    <h3 className="font-semibold text-aether-text">What we’ll set up</h3>
                    <ul className="mt-2 space-y-2 text-sm text-aether-text-muted">
                      <li>Basic profile details and your master resume</li>
                      <li>Claims you confirm as accurate</li>
                      <li>Optional Ollama models and resume presentation preferences</li>
                    </ul>
                  </div>
                </div>
              )}

              {stage === 1 && (
                <div className="grid gap-4 sm:grid-cols-2">
                  <label className="text-sm text-aether-text">
                    Name <span className="text-aether-text-muted">(optional)</span>
                    <input
                      value={profile.full_name}
                      onChange={(event) => setProfile((current) => ({ ...current, full_name: event.target.value }))}
                      autoComplete="name"
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="Your name"
                    />
                  </label>
                  <label className="text-sm text-aether-text">
                    Email <span className="text-aether-text-muted">(optional)</span>
                    <input
                      type="email"
                      value={profile.email}
                      onChange={(event) => setProfile((current) => ({ ...current, email: event.target.value }))}
                      autoComplete="email"
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="you@example.com"
                    />
                  </label>
                  <label className="text-sm text-aether-text">
                    Current or most recent title
                    <input
                      value={profile.current_title ?? ''}
                      onChange={(event) => updateProfile('current_title', event.target.value)}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="Product Manager"
                    />
                  </label>
                  <label className="text-sm text-aether-text">
                    Location
                    <input
                      value={profile.location ?? ''}
                      onChange={(event) => updateProfile('location', event.target.value)}
                      autoComplete="address-level2"
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="City, region"
                    />
                  </label>
                  <label className="text-sm text-aether-text">
                    Phone
                    <input
                      type="tel"
                      value={profile.phone ?? ''}
                      onChange={(event) => updateProfile('phone', event.target.value)}
                      autoComplete="tel"
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="Optional"
                    />
                  </label>
                  <label className="text-sm text-aether-text">
                    Years of experience
                    <input
                      type="number"
                      min="0"
                      max="80"
                      value={profile.years_experience ?? ''}
                      onChange={(event) => setProfile((current) => ({
                        ...current,
                        years_experience: event.target.value ? Number(event.target.value) : null,
                      }))}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="Optional"
                    />
                  </label>
                  <label className="text-sm text-aether-text">
                    LinkedIn
                    <input
                      type="url"
                      value={profile.linkedin ?? ''}
                      onChange={(event) => updateProfile('linkedin', event.target.value)}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="https://linkedin.com/in/you"
                    />
                  </label>
                  <label className="text-sm text-aether-text">
                    GitHub
                    <input
                      type="url"
                      value={profile.github ?? ''}
                      onChange={(event) => updateProfile('github', event.target.value)}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="https://github.com/you"
                    />
                  </label>
                  <label className="text-sm text-aether-text sm:col-span-2">
                    Portfolio
                    <input
                      type="url"
                      value={profile.portfolio ?? ''}
                      onChange={(event) => updateProfile('portfolio', event.target.value)}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="https://your-site.example"
                    />
                  </label>
                  <label className="text-sm text-aether-text sm:col-span-2">
                    Roles you’re exploring
                    <input
                      value={rolesInput}
                      onChange={(event) => setRolesInput(event.target.value)}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      placeholder="Engineering Manager, Staff Engineer"
                    />
                    <span className="mt-1 block text-xs text-aether-text-muted">Separate multiple roles with commas. You can skip any field.</span>
                  </label>
                </div>
              )}

              {stage === 2 && (
                <div>
                  <label htmlFor="master-resume" className="block text-sm font-medium text-aether-text">
                    Resume text
                  </label>
                  <p className="mb-3 mt-1 text-sm text-aether-text-muted">
                    Paste your master resume here. It stays in your local CareerOS workspace; you can also skip this and add it later.
                  </p>
                  <textarea
                    id="master-resume"
                    value={resume}
                    onChange={(event) => setResume(event.target.value)}
                    rows={12}
                    className="w-full resize-y rounded border border-aether-border bg-aether-bg px-3 py-2 text-sm leading-6 text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                    placeholder="Paste your resume content…"
                  />
                  <label className="mt-3 inline-flex cursor-pointer items-center rounded border border-aether-border px-3 py-2 text-sm text-aether-text transition hover:bg-aether-bg focus-within:outline focus-within:outline-2 focus-within:outline-aether-primary">
                    Open a text or Markdown file
                    <input
                      type="file"
                      accept=".txt,.md,text/plain,text/markdown"
                      className="sr-only"
                      onChange={(event) => {
                        const file = event.target.files?.[0];
                        if (!file) return;
                        file.text().then(setResume).catch((err: unknown) => {
                          setError(`Could not read the selected file: ${String(err)}`);
                        });
                        event.target.value = '';
                      }}
                    />
                  </label>
                </div>
              )}

              {stage === 3 && (
                <div>
                  <p className="mb-4 text-sm text-aether-text-muted">
                    These suggestions come from your resume. Nothing is added to your verified claims ledger unless you select it.
                  </p>
                  {claims.length === 0 ? (
                    <div className="rounded-lg border border-aether-border bg-aether-bg p-5">
                      <h3 className="font-semibold text-aether-text">No suggestions to review</h3>
                      <p className="mt-1 text-sm text-aether-text-muted">You can add and verify claims later from your claims ledger.</p>
                    </div>
                  ) : (
                    <fieldset className="space-y-2">
                      <legend className="mb-2 text-sm font-medium text-aether-text">Select only claims you can support with evidence</legend>
                      {claims.map((claim, index) => (
                        <label key={`${claim.statement}-${index}`} className="flex cursor-pointer gap-3 rounded-lg border border-aether-border bg-aether-bg p-3 transition hover:border-aether-primary/60">
                          <input
                            type="checkbox"
                            checked={selectedClaims.includes(index)}
                            onChange={() => toggleClaim(index)}
                            className="mt-1 h-4 w-4 accent-aether-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary"
                          />
                          <span className="min-w-0">
                            <span className="block text-sm text-aether-text">{claim.confirmed_wording}</span>
                            <span className="mt-1 block text-xs capitalize text-aether-text-muted">{claim.category} · from your resume</span>
                          </span>
                        </label>
                      ))}
                    </fieldset>
                  )}
                  {claims.length > 0 && selectedClaims.length === 0 && (
                    <p className="mt-3 text-xs text-aether-text-muted">It’s okay to leave every suggestion unchecked.</p>
                  )}
                </div>
              )}

              {stage === 4 && (
                <div>
                  {models.length === 0 ? (
                    <div className="rounded-lg border border-aether-border bg-aether-bg p-5">
                      <h3 className="font-semibold text-aether-text">No local models found</h3>
                      <p className="mt-1 text-sm text-aether-text-muted">
                        Start Ollama and install a model to use local AI features. You can finish setup now and configure models later in Ollama Settings.
                      </p>
                    </div>
                  ) : (
                    <>
                      <p className="mb-4 text-sm text-aether-text-muted">
                        Select models for local generation. The first selection is used for quick tasks; the second for more complex tasks.
                      </p>
                      <fieldset className="space-y-2">
                        <legend className="sr-only">Available Ollama models</legend>
                        {models.map((model) => (
                          <label key={model.name} className="flex cursor-pointer items-center justify-between gap-4 rounded-lg border border-aether-border bg-aether-bg p-3 transition hover:border-aether-primary/60">
                            <span>
                              <span className="block text-sm font-medium text-aether-text">{model.name}</span>
                              <span className="text-xs text-aether-text-muted">{(model.size / 1024 / 1024 / 1024).toFixed(2)} GB</span>
                            </span>
                            <input
                              type="checkbox"
                              checked={selectedModels.includes(model.name)}
                              onChange={() => toggleModel(model.name)}
                              className="h-4 w-4 accent-aether-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary"
                            />
                          </label>
                        ))}
                      </fieldset>
                    </>
                  )}
                  <div className="mt-5 border-t border-aether-border pt-4">
                    <label htmlFor="new-ollama-model" className="block text-sm font-medium text-aether-text">Install a model</label>
                    <div className="mt-2 flex flex-col gap-2 sm:flex-row">
                      <input
                        id="new-ollama-model"
                        value={newModel}
                        onChange={(event) => setNewModel(event.target.value)}
                        placeholder="For example, llama3.2"
                        className="min-w-0 flex-1 rounded border border-aether-border bg-aether-bg px-3 py-2 text-sm text-aether-text placeholder:text-aether-text-muted focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                      />
                      <button
                        type="button"
                        onClick={pullModel}
                        disabled={loading || !newModel.trim()}
                        className="rounded border border-aether-border px-3 py-2 text-sm text-aether-text transition hover:bg-aether-bg focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary disabled:opacity-50"
                      >
                        Download model
                      </button>
                    </div>
                    <p className="mt-2 text-xs text-aether-text-muted">Downloads can take several minutes. You can also install models later in Ollama Settings.</p>
                  </div>
                  <p className="mt-4 text-xs text-aether-text-muted">
                    Model setup is optional. The guide remains available from the top bar whenever you need it.
                  </p>
                </div>
              )}

              {stage === 5 && (
                <div className="space-y-5">
                  <label className="block text-sm text-aether-text">
                    Resume date format
                    <select
                      value={datePolicy}
                      onChange={(event) => setDatePolicy(event.target.value)}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                    >
                      <option value="month_year">Month and year (e.g. Jan 2022)</option>
                      <option value="year_only">Year only (e.g. 2022)</option>
                    </select>
                  </label>
                  <label className="block text-sm text-aether-text">
                    Tenure wording
                    <select
                      value={tenurePhrasing}
                      onChange={(event) => setTenurePhrasing(event.target.value)}
                      className="mt-1.5 w-full rounded border border-aether-border bg-aether-bg px-3 py-2 text-aether-text focus:border-aether-primary focus:outline-none focus:ring-2 focus:ring-aether-primary/30"
                    >
                      <option value="exact">Keep exact dates and wording</option>
                      <option value="years">Use years of experience only when supported</option>
                    </select>
                  </label>
                  <label className="flex items-start gap-3 rounded-lg border border-aether-border bg-aether-bg p-4 text-sm text-aether-text">
                    <input
                      type="checkbox"
                      checked={employerNeutralMetrics}
                      onChange={(event) => setEmployerNeutralMetrics(event.target.checked)}
                      className="mt-0.5 h-4 w-4 accent-aether-primary focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary"
                    />
                    <span>
                      Keep employer details out of metrics by default
                      <span className="mt-1 block text-xs text-aether-text-muted">A presentation preference only; it does not change or add claims.</span>
                    </span>
                  </label>
                </div>
              )}

              {stage === 6 && (
                <div className="py-4">
                  <div className="flex h-11 w-11 items-center justify-center rounded-full bg-aether-success/15 text-aether-success">
                    <Check size={22} />
                  </div>
                  <h3 className="mt-4 text-lg font-semibold text-aether-text">Setup saved</h3>
                  <p className="mt-2 max-w-xl text-sm leading-6 text-aether-text-muted">
                    Your profile, verified claims, model choices, and presentation preferences are saved. You’re ready to explore CareerOS; the Setup guide remains available from the top bar.
                  </p>
                </div>
              )}
            </>
          )}

          {error && (
            <p className="mt-4 rounded border border-aether-error/40 bg-aether-error/10 p-3 text-sm text-aether-error" role="alert">
              {error}
            </p>
          )}
        </div>

        <footer className="flex items-center justify-between border-t border-aether-border px-6 py-4">
          <div>
            {stage > 0 && stage < stages.length ? (
              <button
                type="button"
                onClick={() => setStage((current) => current - 1)}
                disabled={loading || loadingData}
                className="inline-flex items-center gap-1 rounded px-3 py-2 text-sm text-aether-text-muted transition hover:bg-aether-bg hover:text-aether-text focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary disabled:opacity-50"
              >
                <ChevronLeft size={16} />
                Back
              </button>
            ) : stage === 0 ? (
              <button
                type="button"
                onClick={onClose}
                disabled={loading}
                className="rounded px-3 py-2 text-sm text-aether-text-muted transition hover:bg-aether-bg hover:text-aether-text focus-visible:outline focus-visible:outline-2 focus-visible:outline-aether-primary disabled:opacity-50"
              >
                I’ll explore on my own
              </button>
            ) : null}
          </div>
          <button
            type="button"
            onClick={stepAction}
            disabled={loading || loadingData}
            className="inline-flex items-center gap-2 rounded bg-aether-primary px-4 py-2 text-sm font-medium text-aether-primary-fg transition hover:bg-aether-primary-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-aether-primary disabled:cursor-wait disabled:opacity-50"
          >
            {loading ? (
              <>
                <LoaderCircle size={16} className="animate-spin" />
                Saving…
              </>
            ) : stage === stages.length ? (
              'Open CareerOS'
            ) : stage === stages.length - 1 ? (
              <>
                <Check size={16} />
                Save and finish
              </>
            ) : stage === 0 ? (
              <>
                Get started
                <ChevronRight size={16} />
              </>
            ) : (
              <>
                Continue
                <ChevronRight size={16} />
              </>
            )}
          </button>
        </footer>
      </section>
    </div>
  );
}
