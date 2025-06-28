import { sendAndWait } from '$lib/utils/websocketUtils';
import { MessageType, type Technology } from '$types';

class Technologies {
	private loading = false;

	technologies: Record<string, Technology> = $state({});

	private async ensureTechnologiesLoaded(): Promise<void> {
		if (Object.keys(this.technologies).length === 0 && !this.loading) {
			try {
				this.loading = true;
				this.technologies = await sendAndWait(MessageType.GET_TECHNOLOGIES);
				this.loading = false;
			} catch (error) {
				this.loading = false;
				console.error('Error fetching technologies:', error);
				throw error;
			}
		}
		if (this.loading) {
			await new Promise((resolve) => setTimeout(resolve, 100));
			await this.ensureTechnologiesLoaded();
		}
	}

	async searchTechnologies(search: string): Promise<Technology | undefined> {
		await this.ensureTechnologiesLoaded();
		return this.getByKey(search) || this.getByName(search) || undefined;
	}

	private getByKey(key: string): Technology | undefined {
		return this.technologies[key];
	}

	private getByName(name: string): Technology | undefined {
		return Object.values(this.technologies).find(
			(skill) => skill.localized_name.toLowerCase() === name.toLowerCase()
		);
	}

	async getTechnologies(): Promise<Technology[]> {
		await this.ensureTechnologiesLoaded();
		return Object.values(this.technologies);
	}

	async reset(): Promise<void> {
		this.technologies = {};
		await this.ensureTechnologiesLoaded();
	}
}

export const technologiesData = new Technologies();
