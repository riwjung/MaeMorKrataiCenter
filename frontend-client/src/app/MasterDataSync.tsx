'use client';
import {useEffect} from 'react';
import {loadMasterData} from '../lib/master-data';

// Starts the one master-data fetch for the page (src/lib/master-data.ts). Renders nothing.
export default function MasterDataSync() {
  useEffect(() => { loadMasterData(); }, []);
  return null;
}
