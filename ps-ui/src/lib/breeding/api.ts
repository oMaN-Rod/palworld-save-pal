import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType } from '$types';
import type {
	BreedablePalsResponse,
	DirectChildResponse,
	DirectPartnersResponse,
	DirectParentsResponse,
	ChainRequest,
	ChainResponse
} from './types';

export const breedingApi = {
	breedingPals: () =>
		sendAndWait<BreedablePalsResponse>(MessageType.GET_BREEDING_PALS),

	breedingDirectChild: (params: { parent_a: string; parent_b: string }) =>
		sendAndWait<DirectChildResponse>(MessageType.BREEDING_DIRECT_CHILD, params),

	breedingDirectPartners: (params: { parent_a: string; target_child: string }) =>
		sendAndWait<DirectPartnersResponse>(MessageType.BREEDING_DIRECT_PARTNERS, params),

	breedingDirectParents: (params: { target_child: string }) =>
		sendAndWait<DirectParentsResponse>(MessageType.BREEDING_DIRECT_PARENTS, params),

	breedingChain: (req: ChainRequest) =>
		sendAndWait<ChainResponse>(MessageType.BREEDING_CHAIN, req)
};
