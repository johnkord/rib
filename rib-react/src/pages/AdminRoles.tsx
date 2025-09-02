import { useState } from 'react';
import { useAuth } from '../hooks/useAuth';
import { apiClient } from '../lib/api';

export function AdminRoles() {
  const { user } = useAuth();
  const [discordId, setDiscordId] = useState('');
  const [selectedRole, setSelectedRole] = useState('user');
  const [message, setMessage] = useState('');
  const [error, setError] = useState('');

  // Only admins can access this page
  if (!user || user.role !== 'admin') {
    return (
      <div className="container mx-auto p-4">
        <h1 className="text-2xl font-bold text-red-500">Access Denied</h1>
        <p>This page is only accessible to administrators.</p>
      </div>
    );
  }

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setMessage('');
    setError('');

    try {
      await apiClient.setDiscordRole(discordId, selectedRole);
      setMessage(`Successfully set role for Discord ID ${discordId} to ${selectedRole}`);
      setDiscordId('');
      setSelectedRole('user');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update role');
    }
  };

  return (
    <div className="container mx-auto p-4">
      <h1 className="text-2xl font-bold mb-6">Discord Role Management</h1>
      
      <div className="max-w-md">
        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label htmlFor="discord-id" className="block text-sm font-medium mb-1">
              Discord User ID
            </label>
            <input
              id="discord-id"
              type="text"
              value={discordId}
              onChange={(e) => setDiscordId(e.target.value)}
              placeholder="e.g., 123456789012345678"
              className="w-full p-2 border rounded"
              required
            />
            <p className="text-xs text-gray-500 mt-1">
              You can find this in Discord by enabling Developer Mode and right-clicking a user
            </p>
          </div>

          <div>
            <label htmlFor="role" className="block text-sm font-medium mb-1">
              Role
            </label>
            <select
              id="role"
              value={selectedRole}
              onChange={(e) => setSelectedRole(e.target.value)}
              className="w-full p-2 border rounded"
            >
              <option value="user">User</option>
              <option value="moderator">Moderator</option>
              <option value="admin">Admin</option>
            </select>
          </div>

          <button
            type="submit"
            className="w-full bg-blue-500 text-white py-2 px-4 rounded hover:bg-blue-600"
          >
            Update Role
          </button>
        </form>

        {message && (
          <div className="mt-4 p-3 bg-green-100 text-green-700 rounded">
            {message}
          </div>
        )}

        {error && (
          <div className="mt-4 p-3 bg-red-100 text-red-700 rounded">
            {error}
          </div>
        )}
      </div>

      <div className="mt-8 p-4 bg-gray-100 rounded">
        <h2 className="font-semibold mb-2">How it works:</h2>
        <ul className="text-sm space-y-1">
          <li>• This sets the role for a Discord user before they log in</li>
          <li>• When the user logs in via Discord OAuth, they'll receive this role</li>
          <li>• Roles determine what actions users can perform in the forum</li>
        </ul>
      </div>
    </div>
  );
}
