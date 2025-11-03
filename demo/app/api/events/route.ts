import { NextRequest, NextResponse } from 'next/server';
import { getEvents, type EventQuery } from '../../../src/redis/get-events';

export async function GET(request: NextRequest) {
  try {
    const searchParams = request.nextUrl.searchParams;
    
    const query: EventQuery = {
      eventType: searchParams.get('eventType') as 'buy' | 'sell' | 'burn' | undefined,
      poolAddress: searchParams.get('poolAddress') || undefined,
      userAddress: searchParams.get('userAddress') || undefined,
      startTimestamp: searchParams.get('startTimestamp') 
        ? parseInt(searchParams.get('startTimestamp')!, 10) 
        : undefined,
      endTimestamp: searchParams.get('endTimestamp')
        ? parseInt(searchParams.get('endTimestamp')!, 10)
        : undefined,
      limit: searchParams.get('limit')
        ? parseInt(searchParams.get('limit')!, 10)
        : undefined,
    };

    const events = await getEvents(query);
    
    return NextResponse.json({ events }, { status: 200 });
  } catch (error) {
    console.error('Error fetching events:', error);
    return NextResponse.json(
      { error: 'Failed to fetch events' },
      { status: 500 }
    );
  }
}

