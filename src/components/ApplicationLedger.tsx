import React, { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface ApplicationRecord {
  id: string;
  employer: string;
  role: string;
  req_id: string;
  date: string;
  base_resume_version: string;
  materials_version: string;
  claims_used: string;
  gaps_named: string;
  approved_wording: string;
  salary_entered: string | null;
  status: string;
  notes: string;
}

const STATUS_COLORS: Record<string, string> = {
  PENDING_REVIEW: 'bg-aether-warning text-aether-primary-fg',
  SUBMITTED: 'bg-aether-info text-aether-text',
  INTERVIEWING: 'bg-aether-primary text-aether-primary-fg',
  REJECTED: 'bg-aether-error text-aether-primary-fg',
  OFFER: 'bg-aether-success text-aether-primary-fg',
};

export default function ApplicationLedger() {
  const [records, setRecords] = useState<ApplicationRecord[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    loadRecords();
  }, []);

  const loadRecords = async () => {
    try {
      const data: ApplicationRecord[] = await invoke('get_application_records');
      setRecords(data);
    } catch (err: any) {
      console.error(err);
    } finally {
      setLoading(false);
    }
  };

  const updateStatus = async (id: string, status: string) => {
    try {
      await invoke('update_application_status', { id, status });
      loadRecords();
    } catch (err: any) {
      console.error(err);
    }
  };

  if (loading) {
    return (
      <div className="bg-aether-surface border border-aether-border rounded-xl p-6">
        <h2 className="text-xl font-bold text-aether-text mb-4">Application Ledger</h2>
        <p className="text-aether-text-muted animate-pulse">Loading applications...</p>
      </div>
    );
  }

  return (
    <div className="bg-aether-surface border border-aether-border rounded-xl p-6">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-xl font-bold text-aether-text">Application Ledger</h2>
        <div className="flex gap-2">
          {[ 'PENDING_REVIEW', 'SUBMITTED', 'INTERVIEWING', 'REJECTED', 'OFFER' ].map((s) => (
            <button
              key={s}
              onClick={() => updateStatus('', s)}
              className={`px-3 py-1 rounded text-xs border transition ${STATUS_COLORS[s] || 'border-aether-border text-aether-text-muted'}`}
            >
              {s.replace('_', ' ')}
            </button>
          ))}
        </div>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="border-b border-aether-border">
              {['Employer', 'Role', 'Date', 'Status', 'Claims', 'Gaps'].map((h) => (
                <th key={h} className="pb-3 text-xs font-semibold text-aether-text-muted uppercase tracking-wider">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-aether-border">
            {records.length > 0 ? records.map((r) => (
              <tr key={r.id} className="hover:bg-aether-bg/50 transition">
                <td className="py-4 text-aether-text">{r.employer}</td>
                <td className="py-4 text-aether-text">{r.role}</td>
                <td className="py-4 text-aether-text-muted text-sm">{r.date}</td>
                <td className="py-4">
                  <span className={`px-2 py-1 rounded text-xs font-medium ${STATUS_COLORS[r.status] || 'border-aether-border text-aether-text-muted'}`}>
                    {r.status.replace('_', ' ')}
                  </span>
                </td>
                <td className="py-4 text-aether-text-muted text-sm">{r.claims_used || '0'}</td>
                <td className="py-4 text-aether-text-muted text-sm">{r.gaps_named || '0'}</td>
              </tr>
            )) : (
              <tr>
                <td colSpan={6} className="py-8 text-center text-aether-text-muted">
                  No applications found. Use /apply to start tracking.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      {records.length > 0 && (
        <div className="mt-6 p-4 bg-aether-bg rounded border border-aether-border">
          <h3 className="text-sm font-semibold text-aether-text mb-2">Summary</h3>
          <div className="flex gap-4">
            <div>
              <p className="text-xs text-aether-text-muted">Total</p>
              <p className="text-aether-text font-medium">{records.length}</p>
            </div>
            <div>
              <p className="text-xs text-aether-text-muted">Pending</p>
              <p className="text-aether-warning font-medium">{records.filter(r => r.status === 'PENDING_REVIEW').length}</p>
            </div>
            <div>
              <p className="text-xs text-aether-text-muted">Interviewing</p>
              <p className="text-aether-primary font-medium">{records.filter(r => r.status === 'INTERVIEWING').length}</p>
            </div>
            <div>
              <p className="text-xs text-aether-text-muted">Offers</p>
              <p className="text-aether-success font-medium">{records.filter(r => r.status === 'OFFER').length}</p>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}