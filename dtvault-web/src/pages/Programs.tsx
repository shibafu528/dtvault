import React from 'react';
import { Box, Breadcrumb, BreadcrumbItem, BreadcrumbLink, Container, SimpleGrid } from '@chakra-ui/react';
import { ChevronRightIcon } from '@chakra-ui/icons';

type ItemProps = {
    item: any;
};

const Item: React.FC<ItemProps> = ({ item }) => (
    <Box borderWidth="1px" borderRadius="md" p="2">
        {item.name}
    </Box>
);

function Programs() {
    const items = [{ name: 'foo' }, { name: 'baa' }, { name: 'aaa' }, { name: 'bbb' }];
    return (
        <Container maxW="container.lg" mt="1rem">
            <Breadcrumb spacing="8px" separator={<ChevronRightIcon color="gray.500" />}>
                <BreadcrumbItem isCurrentPage>
                    <BreadcrumbLink>番組一覧</BreadcrumbLink>
                </BreadcrumbItem>
            </Breadcrumb>
            <SimpleGrid columns={3} spacing="20px" mt="1rem">
                {items.map((item) => (
                    <Item item={item} />
                ))}
            </SimpleGrid>
        </Container>
    );
}

export default Programs;
